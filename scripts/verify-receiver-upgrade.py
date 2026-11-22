"""Verify receiver-only upgrades with one unchanged old packaged collector.

Isolated local profiles; includes rollback, live UI files and browser hot reload.
Requires the normal Windows build tools and npm-installed dashboard dependencies.
"""
import argparse
import csv
import datetime
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import time
import urllib.request
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--old-bundle', type=Path, required=True)
    parser.add_argument('--new-bundle', type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    root = repo / 'test_data' / ('receiver-upgrade-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    installed = root / 'installed'
    profile = root / 'receiver'
    installed.mkdir(parents=True)
    old = args.old_bundle.resolve(strict=True)
    new = args.new_bundle.resolve(strict=True)
    for name in ['lmu-engineer.exe', 'lmu-agent.exe']:
        shutil.copy2(old / name, installed / name)
    url = 'http://127.0.0.1:25437'
    env = dict(os.environ, ENGINEER_TEST_RECEIVER=str(installed / 'lmu-engineer.exe'))
    running = []
    logs = []

    def launch(command, **options):
        p = subprocess.Popen(list(map(str, command)), creationflags=subprocess.CREATE_NO_WINDOW, **options)
        running.append(p)
        return p

    def request(path, data=None):
        req = urllib.request.Request(url + path, data=None if data is None else json.dumps(data).encode(),
                                     headers={} if data is None else {'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=10) as response:
            return response.read()

    def state():
        return json.loads(request('/api/state'))

    def ready():
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                return state()
            except OSError:
                time.sleep(.1)
        raise AssertionError('Receiver did not become ready')

    def receiver_pid():
        result = subprocess.check_output(['powershell', '-NoProfile', '-Command',
            "Get-Process -Name lmu-engineer | Where-Object { $_.Path -eq $env:ENGINEER_TEST_RECEIVER } | Select-Object -ExpandProperty Id"], env=env)
        return int(result.strip())

    def update(bundle, success):
        result = subprocess.run(['powershell', '-NoProfile', '-File', str(repo / 'scripts/update-engineer-receiver.ps1'),
            '-BundleDirectory', str(bundle), '-ReceiverDirectory', str(installed), '-DataDirectory', str(profile),
            '-HttpPort', '25437', '-IngestPort', '25443', '-NoTray'], capture_output=True, env=env)
        (root / ('update.log' if success else 'rollback.log')).write_bytes(result.stdout + result.stderr)
        assert (result.returncode == 0) == success, (result.returncode, result.stderr.decode(errors='replace'))
        if not success:
            assert b'previous receiver restored' in result.stderr.lower(), result.stderr.decode(errors='replace')
            assert b'replacement receiver exited during startup' in result.stderr.lower(), result.stderr.decode(errors='replace')
        return ready()

    try:
        launch([installed / 'lmu-engineer.exe', '--data', profile, '--http-port', 25437, '--ingest-port', 25443, '--no-browser', '--no-tray'])
        initial = ready()
        original_event = initial['event_id']
        pairing = request('/api/pair', {'endpoint': '127.0.0.1:25443', 'entry_id': 'Upgrade car', 'label': 'Compatibility driver'})
        driver = root / 'unchanged-driver'
        with zipfile.ZipFile(io.BytesIO(pairing)) as z:
            z.extractall(driver)
        agent = driver / 'lmu-agent.exe'
        original_hash = hashlib.sha256(agent.read_bytes()).hexdigest()
        collector = launch([agent, '--demo', '--seconds', 90, '--no-tray'])
        original_pid = collector.pid
        deadline = time.monotonic() + 20
        while state()['live_batches'] < 8:
            assert time.monotonic() < deadline
            time.sleep(.2)
        before = state()
        original_source = before['sources'][0]['source_id']
        # A valid no-op EXE exits immediately, causing the update helper to restore
        # the old receiver. No crash dialog or game interaction is involved.
        broken = root / 'broken-bundle'
        broken.mkdir()
        shutil.copytree(new / 'ui', broken / 'ui')
        stub = broken / 'noop.rs'
        stub.write_text('#![windows_subsystem="windows"]\nfn main() {}\n')
        subprocess.run(['rustc', str(stub), '-C', 'target-feature=+crt-static', '-o', str(broken / 'lmu-engineer.exe')], check=True, creationflags=subprocess.CREATE_NO_WINDOW)
        rolled_back = update(broken, False)
        assert rolled_back['event_id'] == original_event
        upgraded = update(new, True)
        assert upgraded['event_id'] == original_event and upgraded['sources'][0]['source_id'] == original_source
        assert collector.poll() is None and collector.pid == original_pid
        receiver_identity = receiver_pid()
        index = installed / 'ui/index.html'
        original_index = index.read_bytes()
        for marker in [b'first-live-ui', b'second-live-ui']:
            pending = index.with_suffix('.pending')
            pending.write_bytes(original_index + b'\n<!-- ' + marker + b' -->')
            os.replace(pending, index)
            assert marker in request('/')
            assert receiver_pid() == receiver_identity and collector.poll() is None
        index.write_bytes(original_index)
        # Exercise the real UI publish script while the receiver is serving files.
        published = subprocess.run(['powershell', '-NoProfile', '-File', str(repo / 'scripts/update-engineer-ui.ps1'),
            '-ReceiverDirectory', str(installed), '-BuiltUiDirectory', str(new / 'ui')], capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
        assert published.returncode == 0, published.stderr.decode(errors='replace')
        assert request('/') == (new / 'ui/index.html').read_bytes()
        vite_env = dict(env, ENGINEER_HTTP_PORT='25437', ENGINEER_DEV_PORT='5183',
                        ENGINEER_TEST_DEV_URL='http://127.0.0.1:5183', ENGINEER_TEST_RECEIVER_URL=url)
        log = (root / 'vite.log').open('wb'); logs.append(log)
        launch(['node', repo / 'dashboard/node_modules/vite/bin/vite.js'], cwd=repo / 'dashboard', env=vite_env, stdout=log, stderr=log)
        for _ in range(100):
            try:
                with urllib.request.urlopen('http://127.0.0.1:5183', timeout=1) as response:
                    assert response.status == 200
                break
            except OSError:
                time.sleep(.1)
        browser_log = (root / 'browser.log').open('wb'); logs.append(browser_log)
        subprocess.run(['node', repo / 'dashboard/node_modules/@playwright/test/cli.js', 'test', '--config',
            'dashboard/playwright.config.ts', 'dashboard/tests/receiver-live.spec.ts', '--workers', '1'],
            cwd=repo, env=vite_env, stdout=browser_log, stderr=browser_log, check=True, creationflags=subprocess.CREATE_NO_WINDOW)
        assert collector.pid == original_pid and collector.poll() is None
        collector.wait(timeout=100)
        assert collector.returncode == 0
        final = state()
        assert not final['errors'] and final['event_id'] == original_event
        assert final['sources'][0]['source_id'] == original_source
        rows = list(csv.DictReader(io.StringIO(request('/api/export/Upgrade%20car').decode())))
        assert len(rows) >= 4400
        clocks = [float(row['game_s']) for row in rows]
        gaps = [b-a for a, b in zip(clocks, clocks[1:])]
        assert min(gaps) >= 0 and max(gaps) < .2 and clocks[-1] >= 88
        assert hashlib.sha256(agent.read_bytes()).hexdigest() == original_hash
        assert (installed / 'lmu-agent.exe').read_bytes() == (old / 'lmu-agent.exe').read_bytes()
        db = sqlite3.connect(profile / 'race.sqlite')
        runs, batches, unique = db.execute('select count(distinct run),count(*),count(distinct source||run||seq) from batches').fetchone()
        db.close()
        assert runs == 1 and batches == unique
        result = {'old_bundle': old.name, 'new_bundle': new.name, 'old_collector_unchanged': True,
            'one_collector_process_and_run': True, 'rollback_passed': True, 'pairing_and_certificate_preserved': True,
            'live_ui_replacement_passed': True, 'browser_hot_reload_and_proxy_passed': True,
            'samples': len(rows), 'batches': batches, 'max_game_gap_s': max(gaps), 'errors': []}
        (root / 'result.json').write_text(json.dumps(result, indent=2))
        print(json.dumps(result, indent=2))
    finally:
        for p in running:
            if p.poll() is None:
                p.terminate(); p.wait(timeout=10)
        subprocess.run(['powershell', '-NoProfile', '-Command',
            "Get-Process -Name lmu-engineer -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $env:ENGINEER_TEST_RECEIVER } | Stop-Process"], env=env, creationflags=subprocess.CREATE_NO_WINDOW)
        for log in logs:
            log.close()


if __name__ == '__main__':
    main()
