"""Check the current portable build against a copied full-volume archive."""
import argparse
import csv
import datetime
import io
import json
import shutil
import subprocess
import threading
import time
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('bundle', type=Path)
    parser.add_argument('archive', type=Path)
    parser.add_argument('--reuse', type=Path)
    args = parser.parse_args()
    bundle = args.bundle.resolve(strict=True)
    source = args.archive.resolve(strict=True)
    test_data = Path(__file__).resolve().parent.parent / 'test_data'
    if args.reuse:
        root = args.reuse.resolve(strict=True)
        assert root.is_relative_to(test_data) and (root / 'receiver').is_dir() and root / 'receiver' != source
    else:
        root = test_data / ('large-archive-check-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
        root.mkdir()
        shutil.copytree(source, root / 'receiver')
    processes = []
    def launch(exe, *options):
        process = subprocess.Popen([str(exe), *map(str, options)], creationflags=subprocess.CREATE_NO_WINDOW)
        processes.append(process)
        return process
    def request(path, data=None):
        req = urllib.request.Request('http://127.0.0.1:24437' + path,
                                     data=None if data is None else json.dumps(data).encode(),
                                     headers={} if data is None else {'Content-Type': 'application/json'})
        return urllib.request.urlopen(req, timeout=30)
    def state():
        with request('/api/state') as response:
            return json.load(response)
    try:
        started = time.monotonic()
        host = launch(bundle / 'lmu-engineer.exe', '--data', root / 'receiver', '--http-port', 24437,
                      '--ingest-port', 24443, '--no-tray', '--no-browser')
        (root / 'receiver-pid.txt').write_text(str(host.pid))
        while True:
            try:
                restored = state()
                break
            except Exception:
                assert host.poll() is None and time.monotonic() - started < 180
                time.sleep(.2)
        restore_s = time.monotonic() - started
        assert len(restored['entries']) == 8 and not restored['errors']
        assert all(e['last_received_ms'] == 0 and len(e['laps']) == 100 for e in restored['entries'])
        for entry in restored['entries']:
            with request('/api/laps/' + urllib.parse.quote(entry['entry_id'], safe='')) as response:
                assert len(json.load(response)) == 179
        assert all(e['strategy']['clean_laps'] >= 3 and not e['strategy']['scenarios'] for e in restored['entries'])
        print(json.dumps({'restored_s': restore_s, 'cars': 8, 'pid': host.pid, 'root': str(root)}), flush=True)
        with request('/api/pair', {'endpoint': '127.0.0.1:24443', 'entry_id': 'Export live check', 'label': 'Archive load test'}) as response:
            data = response.read()
        driver = root / 'driver'
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            archive.extractall(driver)
        collector = launch(driver / 'lmu-agent.exe', '--no-tray', '--demo', '--seconds', 30)
        export = {}
        failures = []
        def download():
            try:
                before = time.monotonic()
                count = 0
                last = None
                first = None
                largest_gap = 0
                names = set()
                path = '/api/export/' + urllib.parse.quote(restored['entries'][0]['entry_id'], safe='')
                with request(path) as response:
                    for row in csv.DictReader(io.TextIOWrapper(response, encoding='utf-8')):
                        game = float(row['game_s'])
                        if last is not None:
                            assert game >= last, 'Export reversed the game clock'
                            largest_gap = max(largest_gap, game - last)
                        else:
                            first = game
                        last = game
                        count += 1
                        names.add(row['driver'])
                export.update({'samples': count, 'first_game_s': first, 'last_game_s': last,
                               'max_game_gap_s': largest_gap, 'drivers': sorted(names),
                               'duration_s': time.monotonic() - before})
                (root / 'export-diagnostic.json').write_text(json.dumps(export, indent=2), encoding='utf-8')
                assert count == 1080000 and first == 0 and last == 21600, export
                # The source benchmark deliberately moves its final 21599.98 s
                # sample to 21600 s to complete the final lap transition.
                assert largest_gap <= .040001 and len(names) == 2, export
            except Exception as error:
                failures.append(repr(error))
        worker = threading.Thread(target=download)
        worker.start()
        latencies = []
        live_seen = False
        deadline = time.monotonic() + 150
        while worker.is_alive() or collector.poll() is None:
            assert time.monotonic() < deadline, 'Export/capture did not finish'
            before = time.monotonic()
            board = state()
            latencies.append(time.monotonic() - before)
            assert not board['errors']
            live = next((e for e in board['entries'] if e['entry_id'] == 'Export live check'), None)
            if live and time.time()*1000-live['last_received_ms'] < 3000:
                live_seen = True
            with (root / 'progress.jsonl').open('a', encoding='utf-8') as out:
                out.write(json.dumps({'elapsed_s': time.monotonic()-started, 'exporting': worker.is_alive(),
                                      'live_game_s': None if live is None else live['last_sample']['game_s'],
                                      'state_latency_s': latencies[-1], 'errors': board['errors']}) + '\n')
            time.sleep(.5)
        worker.join()
        assert not failures, failures
        assert live_seen and collector.returncode == 0
        assert max(latencies) < 2
        final = state()
        live = next(e for e in final['entries'] if e['entry_id'] == 'Export live check')
        assert live['last_sample']['game_s'] > 28
        result = {'bundle': str(bundle), 'source_archive': str(source), 'restored_cars': 8,
                  'archive_samples': 8640000, 'restore_s': restore_s, 'export': export,
                  'live_capture_during_export': True, 'live_collector_drained': True,
                  'max_state_latency_s': max(latencies), 'errors': []}
        (root / 'result.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
        print(json.dumps(result, indent=2), flush=True)
    finally:
        for process in processes:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=15)


if __name__ == '__main__':
    main()
