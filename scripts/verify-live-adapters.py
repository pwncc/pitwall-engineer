"""Check captured native/legacy frames from the same real offline car."""
import json
from pathlib import Path
root=Path(__file__).resolve().parent.parent/'test_data'
entries=json.loads((root/'live-adapter-comparison.json').read_text(encoding='utf-8-sig'))
native=next(e['last_sample'] for e in entries if e['entry_id']=='Adapter native')
legacy=next(e['last_sample'] for e in entries if e['entry_id']=='Adapter legacy')
assert native['authoritative'] and legacy['authoritative']
assert native['track']==legacy['track'] and native['vehicle']==legacy['vehicle']
assert abs(native['captured_ms']-legacy['captured_ms'])<1500
assert native['fuel_l']==legacy['fuel_l']==106
assert native['gear']==legacy['gear']==0
assert abs(native['brake_bias_front']-legacy['brake_bias_front'])<1e-6
assert native['ve']==1 and legacy['ve'] is None
for a,b in zip(native['wheels'],legacy['wheels']):
 assert abs(a['pressure_kpa']-b['pressure_kpa'])<1
 assert abs(a['brake_c']-b['brake_c'])<2
 assert max(abs(x-y) for x,y in zip(a['temperature_c'],b['temperature_c']))<2
sup=native['supplemental'];repair=sup['wearables']
assert repair['weatherForecast']['nodes']['Temperature']==[29,30,30,30,31]
assert repair['weatherForecast']['nodes']['RainChance']==[0]*5
assert max(repair['weatherForecast']['nodes']['StartTime'])==0
assert all(abs(a-b)<1e-6 for a,b in zip(repair['wearables']['brakes'],[.04,.04,.035,.035]))
assert sup['pit_estimate']['total']==0
result={'track':native['track'],'vehicle':native['vehicle'],'plugin_file_version':'3.7.15.1',
        'matching_channels':['fuel','gear','brake_bias','tyre_pressure','tyre_temperature','brake_temperature'],
        'native_virtual_energy':1,'legacy_virtual_energy':None,'forecast_air_c':[29,30,30,30,31],
        'forecast_timestamps_available':False,'brake_thickness_mm':[40,40,35,35],
        'selected_pit_service_s':0,'errors':[]}
(root/'live-adapter-verification.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
