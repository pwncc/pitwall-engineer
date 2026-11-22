"""Summarize actual recorded samples from the receiver's CSV export."""
import csv
import io
import json
import statistics
import urllib.parse
import urllib.request
from collections import Counter
from pathlib import Path

url = 'http://127.0.0.1:7437'
with urllib.request.urlopen(url + '/api/state', timeout=10) as response:
    board = json.load(response)
results = []
for entry in board['entries']:
    with urllib.request.urlopen(url + '/api/export/' + urllib.parse.quote(entry['entry_id'], safe=''), timeout=30) as response:
        raw = response.read()
    root = Path(__file__).resolve().parent.parent / 'test_data'
    (root / 'actual-solo-telemetry.csv').write_bytes(raw)
    rows = list(csv.DictReader(io.StringIO(raw.decode())))
    steps = [int(b['captured_ms']) - int(a['captured_ms']) for a, b in zip(rows, rows[1:])]
    positive = sorted(step for step in steps if step > 0)
    game_steps = [float(b['game_s']) - float(a['game_s']) for a, b in zip(rows, rows[1:])]
    result = {
        'entry': entry['entry_id'], 'track': entry['last_sample']['track'],
        'vehicle': entry['last_sample']['vehicle'], 'samples': len(rows),
        'samples_per_lap_number': dict(Counter(row['lap'] for row in rows)),
        'median_capture_step_ms': statistics.median(positive) if positive else None,
        'p99_capture_step_ms': positive[min(len(positive)-1, int(len(positive)*.99))] if positive else None,
        'capture_gaps_over_1s': sum(step > 1000 for step in steps),
        'capture_gap_details': [
            {'wall_ms': step, 'game_s': game_steps[index], 'lap': rows[index+1]['lap']}
            for index, step in enumerate(steps) if step > 1000
        ],
        'game_clock_resets': sum(step < 0 for step in game_steps),
        'game_clock_gaps_over_1s': sum(step > 1 for step in game_steps),
        'recorded_laps': entry['laps'], 'strategy': entry['strategy'],
        'receiver_errors': board['errors'],
    }
    for field in ['speed_kmh', 'fuel_l', 've', 'throttle', 'brake', 'steering']:
        values = [float(row[field]) for row in rows if row[field]]
        result[field + '_range'] = [min(values), max(values)] if values else None
    results.append(result)
(Path(__file__).resolve().parent.parent / 'test_data' / 'actual-solo-validation.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
print(json.dumps(results, indent=2))
