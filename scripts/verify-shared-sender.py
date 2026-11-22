"""Run one byte-identical sender EXE under three isolated Windows profiles."""
import argparse, csv, datetime, hashlib, io, json, os, sqlite3, struct, subprocess, time, urllib.request, zipfile
from pathlib import Path

def main():
    parser=argparse.ArgumentParser();parser.add_argument('bundle',type=Path);args=parser.parse_args()
    bundle=args.bundle.resolve(strict=True)
    root=Path(__file__).resolve().parent.parent/'test_data'/('shared-sender-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    root.mkdir(parents=True);url='http://127.0.0.1:26437';processes=[]
    def launch(command,**options):
        p=subprocess.Popen(list(map(str,command)),creationflags=subprocess.CREATE_NO_WINDOW,**options);processes.append(p);return p
    def request(path,data=None):
        req=urllib.request.Request(url+path,data=None if data is None else json.dumps(data).encode(),headers={} if data is None else {'Content-Type':'application/json'})
        with urllib.request.urlopen(req,timeout=10) as response:return response.read()
    def board():return json.loads(request('/api/state'))
    try:
        launch([bundle/'lmu-engineer.exe','--data',root/'receiver','--http-port',26437,'--ingest-port',26443,'--no-browser','--no-tray'])
        for _ in range(100):
            try:board();break
            except OSError:time.sleep(.1)
        data=request('/api/pair',{'endpoint':'127.0.0.1:26443','entry_id':'Shared race car','label':'Race team','shared_package':True})
        folder=root/'one-common-package'
        with zipfile.ZipFile(io.BytesIO(data)) as archive:archive.extractall(folder)
        exe=folder/'lmu-agent.exe';original=exe.read_bytes();digest=hashlib.sha256(original).hexdigest()
        length=struct.unpack('<I',original[-12:-8])[0];template=json.loads(original[-12-length:-12]);assert template['shared_package']
        profiles=[root/f'pc-{n}' for n in range(3)];first_ids=None
        for seconds in [12,5]:
            collectors=[]
            for n,profile in enumerate(profiles):
                profile.mkdir(exist_ok=True)
                env=dict(os.environ,LOCALAPPDATA=str(profile),COMPUTERNAME=f'SIMULATED-PC-{n}')
                collectors.append(launch([exe,'--demo','--seconds',seconds,'--no-tray'],env=env))
            for p in collectors:assert p.wait(timeout=40)==0
            state=board();assert not state['errors'],state['errors']
            children=[s for s in state['sources'] if s.get('parent_source_id')==template['source_id']]
            ids={s['source_id'] for s in children};assert len(children)==len(ids)==3
            assert len(state['entries'])==1 and state['entries'][0]['entry_id']=='Shared race car'
            if first_ids is None:first_ids=ids
            else:assert first_ids==ids,'Relaunch created new PC identities'
            for profile in profiles:
                saved=json.loads((profile/'LMUEngineerAgent/identities'/(template['source_id']+'.json')).read_text())
                assert saved['source_id'] in ids
                assert not list((profile/'LMUEngineerAgent'/saved['source_id']/'spool').glob('*.batch'))
        assert hashlib.sha256(exe.read_bytes()).hexdigest()==digest
        rows=list(csv.DictReader(io.StringIO(request('/api/export/Shared%20race%20car').decode())))
        counts={source:sum(row['source_id']==source for row in rows) for source in first_ids}
        assert all(count>=840 for count in counts.values()),counts
        db=sqlite3.connect(root/'receiver/race.sqlite');batches,unique=db.execute('select count(*),count(distinct source||run||seq) from batches').fetchone();db.close();assert batches==unique
        request('/api/revoke/'+template['source_id'],{});state=board()
        assert len(state['sources'])==4 and all(s['revoked'] for s in state['sources'])
        result={'identical_sender_exe':True,'pcs':3,'persistent_pc_identities':True,'one_shared_car':True,'samples_per_pc':sorted(counts.values()),'batches':batches,'duplicate_batches':0,'whole_package_revocation_passed':True,'errors':[]}
        (root/'result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
    finally:
        for p in processes:
            if p.poll() is None:p.terminate();p.wait(timeout=10)

if __name__=='__main__':main()
