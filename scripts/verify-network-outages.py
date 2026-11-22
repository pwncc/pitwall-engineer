"""Run packaged collectors through a delayed TCP relay with forced outages.

The relay forwards encrypted bytes unchanged. All profiles and recordings are
isolated synthetic fixtures. This is a transport fault test, not a WAN probe.
"""
import argparse
import asyncio
import csv
import datetime
import io
import json
import os
from pathlib import Path
import struct
import subprocess
import threading
import time
import urllib.parse
import urllib.request
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--endpoint-host', default='127.0.0.1')
    args = parser.parse_args()
    bundle = args.bundle.resolve(strict=True)
    root = Path(__file__).resolve().parent.parent / 'test_data' / ('network-outage-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    root.mkdir(parents=True)
    started = time.monotonic()
    stop = threading.Event()
    relay_stats = {'connections': 0, 'drops': 0, 'bytes': 0}

    async def relay(reader, writer):
        relay_stats['connections'] += 1
        remote = None
        tasks = []
        accepted = time.monotonic()
        def outage():
            elapsed = time.monotonic() - started
            return 60 <= elapsed < 120 or 240 <= elapsed < 300
        try:
            if outage():
                return
            upstream, remote = await asyncio.open_connection('127.0.0.1', 23443)
            async def copy(source, destination):
                while data := await source.read(65536):
                    await asyncio.sleep(.12)
                    destination.write(data)
                    await destination.drain()
                    relay_stats['bytes'] += len(data)
            tasks = [asyncio.create_task(copy(reader, remote)), asyncio.create_task(copy(upstream, writer))]
            while not any(t.done() for t in tasks) and not stop.is_set():
                elapsed = time.monotonic() - started
                # Terminate established TLS sessions, including those already
                # alive when an outage begins, without touching payload bytes.
                if outage() or (30 <= elapsed < 31 and accepted - started < 30) or (180 <= elapsed < 181 and accepted - started < 180):
                    relay_stats['drops'] += 1
                    break
                await asyncio.sleep(.05)
        except (ConnectionError, OSError):
            pass
        finally:
            for task in tasks:
                task.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)
            writer.close()
            if remote:
                remote.close()

    async def serve():
        server = await asyncio.start_server(relay, '0.0.0.0', 23444)
        async with server:
            while not stop.is_set():
                await asyncio.sleep(.1)
    thread = threading.Thread(target=lambda: asyncio.run(serve()), daemon=True)
    processes = []
    def launch(exe, *options):
        process = subprocess.Popen([str(exe), *map(str, options)], creationflags=subprocess.CREATE_NO_WINDOW)
        processes.append(process)
        return process
    def request(path, data=None):
        req = urllib.request.Request('http://127.0.0.1:23437' + path,
                                     data=None if data is None else json.dumps(data).encode(),
                                     headers={} if data is None else {'Content-Type': 'application/json'})
        return urllib.request.urlopen(req, timeout=30).read()
    def state():
        return json.loads(request('/api/state'))
    try:
        receiver = launch(bundle / 'lmu-engineer.exe', '--data', root / 'receiver', '--http-port', 23437,
                          '--ingest-port', 23443, '--no-browser', '--no-tray')
        deadline = time.monotonic() + 60
        while True:
            try:
                state()
                break
            except Exception:
                assert receiver.poll() is None and time.monotonic() < deadline
                time.sleep(.2)
        thread.start()
        collectors = []
        profiles = []
        for number in range(3):
            data = request('/api/pair', {'endpoint': f'{args.endpoint_host}:23444', 'entry_id': f'Fault car {number}', 'label': f'Fault driver {number}'})
            folder = root / f'driver-{number}'
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                archive.extractall(folder)
            raw = (folder / 'lmu-agent.exe').read_bytes()
            size = struct.unpack('<I', raw[-12:-8])[0]
            config = json.loads(raw[-12-size:-12])
            profiles.append(Path(os.environ['LOCALAPPDATA']) / 'LMUEngineerAgent' / config['source_id'])
            collectors.append(launch(folder / 'lmu-agent.exe', '--no-tray', '--demo', '--seconds', 900, '--speed', 1))
        previous = {}
        max_spool = 0
        stale_observed = False
        recovered = False
        errors = []
        deadline = time.monotonic() + 1100
        while any(p.poll() is None for p in collectors):
            assert time.monotonic() < deadline, 'Collectors failed to drain'
            board = state()
            errors.extend(board['errors'])
            now = int(time.time() * 1000)
            for entry in board['entries']:
                ident = entry['entry_id']
                game = entry['last_sample']['game_s']
                assert game >= previous.get(ident, 0), 'Historical backfill replaced live values'
                previous[ident] = game
                if now - entry['last_received_ms'] > 5000:
                    stale_observed = True
                if time.monotonic() - started > 310 and now - entry['last_received_ms'] < 3000:
                    recovered = True
            queued = sum(len(list((p / 'spool').glob('*.batch'))) for p in profiles)
            max_spool = max(max_spool, queued)
            with (root / 'progress.jsonl').open('a', encoding='utf-8') as out:
                out.write(json.dumps({'elapsed_s': time.monotonic() - started, 'queued': queued, 'game_s': previous, 'errors': board['errors']}) + '\n')
            time.sleep(1)
        assert all(p.returncode == 0 for p in collectors)
        assert stale_observed and recovered and max_spool >= 100
        assert not errors
        assert all(not list((p / 'spool').glob('*.batch')) for p in profiles)
        final = state()
        exports = []
        for entry in final['entries']:
            data = request('/api/export/' + urllib.parse.quote(entry['entry_id'], safe='')).decode()
            (root / (entry['entry_id'].replace(' ', '-') + '.csv')).write_text(data, encoding='utf-8')
            rows = list(csv.DictReader(io.StringIO(data)))
            clocks = [float(row['game_s']) for row in rows]
            gaps = [b-a for a, b in zip(clocks, clocks[1:])]
            assert clocks[-1] >= 898 and min(gaps) >= 0 and max(gaps) < 1, 'Missing or misordered recording'
            assert entry['strategy']['clean_laps'] >= 3
            exports.append({'entry': entry['entry_id'], 'samples': len(rows), 'max_game_gap_s': max(gaps), 'clean_laps': entry['strategy']['clean_laps']})
        result = {'bundle': str(bundle), 'duration_s': 900, 'one_way_delay_s': .12, 'collector_endpoint_host': args.endpoint_host,
                  'outage_windows_s': [[60, 120], [240, 300]], 'max_queued_batches': max_spool,
                  'stale_observed': stale_observed, 'recovered': recovered, 'exports': exports,
                  'relay': relay_stats, 'errors': errors}
        (root / 'result.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
        print(json.dumps(result, indent=2), flush=True)
    finally:
        stop.set()
        for process in processes:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=15)
        if thread.is_alive():
            thread.join(timeout=5)


if __name__ == '__main__':
    main()
