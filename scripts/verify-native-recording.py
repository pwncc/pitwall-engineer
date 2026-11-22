"""Check completed game-generated native laps against an isolated receiver."""
import hashlib,json,urllib.request
from pathlib import Path

root=Path(__file__).resolve().parent.parent/'test_data'
recording=root/'public-paul-ricard.duckdb'
url='http://127.0.0.1:17437'
def request(path,data=None):
    call=urllib.request.Request(url+path,data=None if data is None else json.dumps(data).encode(),headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(call,timeout=30) as response:return json.load(response)
before=hashlib.sha256(recording.read_bytes()).hexdigest()
inspection=request('/api/native/inspect',{'path':str(recording)})
board=request('/api/state');entry=board['entries'][0]
history=[(lap['id'],lap['valid'],lap['fuel_used_l']) for lap in entry['laps']]
assert len(inspection['laps'])>=3
assert inspection['unit_hints']['Ground Speed']=='km/h'
assert inspection['unit_hints']['Throttle Pos']=='%'
results=[]
for lap in inspection['laps']:
    imported=request('/api/native/import',{'path':str(recording),'entry_id':entry['entry_id'],
        'start_s':lap['start_s'],'end_s':lap['end_s'],'speed_unit':'km/h','input_scale':100})
    points=imported['samples']
    assert imported['reference_only'] and not imported['valid']
    assert imported['fuel_used_l'] is None and imported['ve_used'] is None
    assert len(points)>1000
    assert abs(imported['time_s']-lap['time_s'])<1e-6
    assert all(0<=p['throttle']<=1.001 and 0<=p['brake']<=1.001 and abs(p['steering'])<=1.01 for p in points)
    assert all(b['lap_distance_m']+10>=a['lap_distance_m'] for a,b in zip(points,points[1:]))
    assert points[0]['lap_distance_m']<30 and points[-1]['lap_distance_m']>5500
    assert max(p['speed_kmh'] for p in points)>250
    assert all(p['ve'] is None or 0<=p['ve']<=1 for p in points)
    (root/('public-native-lap-'+imported['id']+'.json')).write_text(json.dumps(imported),encoding='utf-8')
    results.append({'time_s':imported['time_s'],'samples':len(points),'first_distance_m':points[0]['lap_distance_m'],
                    'last_distance_m':points[-1]['lap_distance_m'],'speed_max_kmh':max(p['speed_kmh'] for p in points)})
after=request('/api/state')['entries'][0]
assert [(lap['id'],lap['valid'],lap['fuel_used_l']) for lap in after['laps']]==history
assert hashlib.sha256(recording.read_bytes()).hexdigest()==before
result={'source':'https://github.com/lamerman/telemetry-visualizer-for-lmu/blob/c3338a8dcb03c62b0c707fcbf005fa9f945ded2d/data/Paul%20Ricard%20Circuit_P_2026-04-03T20_06_03Z.duckdb',
        'sha256':before,'source_unchanged':True,'live_history_unchanged':True,'laps':results,'errors':[]}
(root/'public-native-import-verification.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
