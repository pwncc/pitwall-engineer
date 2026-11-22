"""Compare exported scalar channels with the actual archived final LMU sample."""
import csv
import io
import json
import urllib.parse
import urllib.request
from pathlib import Path

base = 'http://127.0.0.1:7437'
with urllib.request.urlopen(base + '/api/state', timeout=20) as response:
    state = json.load(response)
entry = next(e for e in state['entries'] if e['entry_id'] == 'Local car')
sample = entry['last_sample']
found = None
count = 0
contexts = set()
garage_rows = 0
with urllib.request.urlopen(base + '/api/export/' + urllib.parse.quote(entry['entry_id'], safe=''), timeout=30) as response:
    reader = csv.DictReader(io.TextIOWrapper(response, encoding='utf-8'))
    assert len(reader.fieldnames) == 118 and len(set(reader.fieldnames)) == 118
    for row in reader:
        assert None not in row and all(value is not None for value in row.values()), 'CSV field count mismatch'
        count += 1
        contexts.add((row['track'], row['vehicle']))
        if row['authoritative'] == 'false':
            garage_rows += 1
            assert row['fuel_l'] == '' and row['ve'] == ''
        if int(row['captured_ms']) == sample['captured_ms']:
            found = row
assert found and count > 190000 and garage_rows > 100 and len(contexts) >= 2
assert found['track'] == sample['track'] and found['vehicle'] == sample['vehicle'] and found['driver'] == sample['driver']
assert found['source_id'] == entry['source_id'] and found['entry_id'] == entry['entry_id']
checked = []
def numeric(column, expected):
    if expected is None:
        assert found[column] == '', column
    else:
        assert abs(float(found[column])-expected) < 1e-8, (column, found[column], expected)
    checked.append(column)
for column, key in [('fuel_l', 'fuel_l'), ('ve', 've'), ('fuel_capacity_l', 'fuel_capacity_l'),
                    ('soc', 'soc'), ('water_c', 'water_c'), ('oil_c', 'oil_c'), ('brake_bias_front', 'brake_bias_front'),
                    ('track_length_m', 'track_length_m'), ('rpm', 'rpm'), ('gear', 'gear'), ('tc', 'tc'), ('abs', 'abs'),
                    ('rain', 'rain'), ('wetness', 'wetness'), ('air_c', 'air_c'), ('track_c', 'track_c')]:
    numeric(column, sample[key])
for prefix, wheel in zip(['fl', 'fr', 'rl', 'rr'], sample['wheels']):
    for name in ['carcass_c', 'pressure_kpa', 'remaining', 'brake_c', 'compound', 'suspension_m', 'ride_height_m']:
        numeric(prefix + '_' + name, wheel[name])
    for index, name in enumerate(['left', 'center', 'right']):
        numeric(prefix + '_temperature_' + name + '_c', None if wheel['temperature_c'] is None else wheel['temperature_c'][index])
assert found['authoritative'] == str(sample['authoritative']).lower()
assert found['valid_lap'] == str(sample['valid_lap']).lower()
assert found['in_pits'] == str(sample['in_pits']).lower()
result = {'samples': count, 'columns': 118, 'contexts': sorted(contexts), 'garage_rows': garage_rows,
          'checked_numeric_channels': checked, 'matched_final_sample_ms': sample['captured_ms'], 'errors': []}
root = Path(__file__).resolve().parent.parent / 'test_data'
(root / 'real-export-verification.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
print(json.dumps(result, indent=2))
