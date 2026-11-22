"""Cross-check the recorded Bahrain laps against their archived raw traces."""
import json
import statistics
import urllib.parse
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parent.parent / 'test_data'
report = json.loads((root/'bahrain-completed-validation.json').read_text(encoding='utf-8'))[0]
laps = report['recorded_laps']
clean = [lap for lap in laps if lap['valid'] and not lap['reference_only']][:15]
assert len(clean) >= 3
expected_fuel = statistics.median(lap['fuel_used_l'] for lap in clean if lap['fuel_used_l'] is not None)
expected_pace = statistics.median(lap['time_s'] for lap in clean)
assert abs(expected_fuel-report['strategy']['fuel_per_lap']) < 1e-9
assert abs(expected_pace-report['strategy']['pace_s']) < 1e-9
destination = root/'verified-bahrain-laps'
destination.mkdir(exist_ok=True)
checked = []
for lap in clean:
    with urllib.request.urlopen('http://127.0.0.1:7437/api/lap/'+urllib.parse.quote(lap['id'], safe=''), timeout=20) as response:
        full = json.load(response)
    samples = full['samples']
    assert len(samples) > 100, lap['id']
    assert all(sample['authoritative'] and sample['valid_lap'] and not sample['in_pits'] and not sample['yellow'] for sample in samples)
    assert all(sample['track'] == report['track'] and sample['vehicle'] == report['vehicle'] for sample in samples)
    assert samples[0]['lap_distance_m'] < 80
    assert samples[-1]['lap_distance_m'] > samples[-1]['track_length_m'] - 100
    assert all(b['game_s'] >= a['game_s'] for a,b in zip(samples,samples[1:]))
    elapsed = samples[-1]['game_s'] - samples[0]['game_s']
    assert abs(elapsed - lap['time_s']) < .5, (lap['lap'], elapsed, lap['time_s'])
    raw_fuel = samples[0]['fuel_l'] - samples[-1]['fuel_l']
    assert abs(raw_fuel-lap['fuel_used_l']) < .05, (lap['lap'],raw_fuel,lap['fuel_used_l'])
    (destination/(lap['id']+'.json')).write_text(json.dumps(full),encoding='utf-8')
    checked.append({'lap':lap['lap'],'samples':len(samples),'official_time_s':lap['time_s'],
                    'trace_extent_s':elapsed,'fuel_used_l':lap['fuel_used_l'],'trace_fuel_used_l':raw_fuel})
result={'clean_laps_checked':len(checked),'median_fuel_l':expected_fuel,'median_pace_s':expected_pace,'laps':checked}
(root/'real-lap-verification.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
