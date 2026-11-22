"""Record receiver observations without altering game or receiver state."""
import datetime
import json
import time
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parent.parent / 'test_data'
path = root / ('live-session-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S') + '.jsonl')
print(str(path), flush=True)
deadline = time.monotonic() + 1200
last_print = 0
last_signature = None
while time.monotonic() < deadline:
    record = {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        with urllib.request.urlopen('http://127.0.0.1:7437/api/state', timeout=5) as response:
            board = json.load(response)
        record.update(batches=board.get('live_batches'), errors=board.get('errors'), entries=[])
        for entry in board['entries']:
            sample = entry['last_sample']
            record['entries'].append({
                'entry': entry['entry_id'],
                'sample': {key: sample.get(key) for key in (
                    'captured_ms', 'game_s', 'track', 'track_length_m', 'session_kind',
                    'session_remaining_s', 'driver', 'vehicle', 'class', 'lap',
                    'lap_distance_m', 'lap_start_s', 'last_lap_s', 'authoritative',
                    'valid_lap', 'in_pits', 'speed_kmh', 'fuel_l', 'fuel_capacity_l',
                    've', 'soc', 'rpm', 'gear', 'throttle', 'brake', 'wheels', 'yellow')},
                'laps': entry.get('laps'), 'strategy': entry.get('strategy'),
            })
        signature = [(e['entry'], e['sample']['track'], e['sample']['lap'],
                      e['sample']['authoritative'], len(e['laps'] or [])) for e in record['entries']]
        if signature != last_signature or time.monotonic() - last_print >= 30:
            print(json.dumps({'utc': record['utc'], 'batches': record['batches'],
                              'entries': signature, 'errors': record['errors']}), flush=True)
            last_print = time.monotonic()
            last_signature = signature
    except Exception as error:
        record['request_error'] = str(error)
        print(json.dumps(record), flush=True)
    with path.open('a', encoding='utf-8') as output:
        output.write(json.dumps(record) + '\n')
    time.sleep(5)
