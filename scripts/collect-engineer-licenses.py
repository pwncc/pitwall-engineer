"""Collect installed dependency notices for the private Windows release bundle."""
import json, subprocess, sys, shutil
from pathlib import Path
repo=Path(__file__).resolve().parent.parent
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
metadata=json.loads(subprocess.check_output(['cargo','metadata','--manifest-path',str(repo/'bridge/Cargo.toml'),'--format-version','1','--locked'],text=True,encoding='utf-8'))
index=[]
def collect(kind,name,version,root,license_name,source):
 target=out/kind/(name.replace('/','_')+'-'+version)
 files=[]
 for path in root.iterdir():
  if path.is_file() and path.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','OFL')):
   target.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,target/path.name);files.append(path.name)
 index.append(dict(kind=kind,name=name,version=version,license=license_name,source=source,notices=files))
for package in metadata['packages']:
 collect('rust',package['name'],package['version'],Path(package['manifest_path']).parent,package.get('license'),package.get('repository'))
lock=json.loads((repo/'dashboard/package-lock.json').read_text(encoding='utf-8'))
for relative,package in lock.get('packages',{}).items():
 if not relative:continue
 root=repo/'dashboard'/relative
 if not root.is_dir():continue
 manifest=root/'package.json'
 if not manifest.exists():continue
 info=json.loads(manifest.read_text(encoding='utf-8'))
 collect('javascript',info.get('name',root.name),info.get('version','unknown'),root,info.get('license'),info.get('repository'))
(out/'dependency-index.json').write_text(json.dumps(index,indent=2),encoding='utf-8')
print(f'Collected notices for {len(index)} installed dependencies')
