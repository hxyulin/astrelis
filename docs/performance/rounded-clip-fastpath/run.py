import subprocess, pathlib, datetime, json, hashlib, platform
out = pathlib.Path('docs/performance/rounded-clip-fastpath')
bins = pathlib.Path('/tmp/astrelis-clip-fastpath-bin')
def capture(cmd):
    return subprocess.check_output(cmd, text=True, stderr=subprocess.STDOUT)
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def snapshot(name):
    (out / (name + '.host.txt')).write_text(datetime.datetime.now(datetime.timezone.utc).isoformat()+'\n'+capture(['uptime'])+capture(['ps','-Ao','pid,ppid,pcpu,pmem,comm']))
metadata = {'baseline':capture(['git','rev-parse','HEAD']).strip(), 'original':'5e5ea9df12f3539661b8d487a83b907e63641bfa', 'platform':platform.platform(), 'os':capture(['sw_vers']), 'hardware':capture(['sysctl','hw.model','hw.memsize','hw.ncpu','machdep.cpu.brand_string']), 'rust':capture(['rustc','-Vv']), 'cargo':capture(['cargo','-V']), 'binary_sha256':{p.name:digest(p) for p in bins.iterdir()}, 'source_sha256':{p:digest(pathlib.Path(p)) for p in ['Cargo.lock','crates/astrelis/benches/primitives.rs','crates/astrelis/benches/mixed_ui.rs','crates/astrelis/src/context.rs','crates/astrelis/src/clip.rs','crates/astrelis/src/pass.rs']}, 'original_normalized_lock_sha256':digest(pathlib.Path('/tmp/astrelis-clip-fastpath-original/Cargo.lock')), 'runs':[]}
(out/'source.patch').write_text(capture(['git','diff','--','crates/astrelis/src']))
for round in range(1,4):
    # Reverse each adjacent before/after pair on the second round. Include the
    # original library as context using exactly the same benchmark source.
    order = ['before','after','original'] if round % 2 else ['after','before','original']
    for side in order:
        name=f'primitives-{side}-{round}'
        cmd=[str(bins/f'primitives-{side}'),'--counts','100,1000,10000','--samples','25','--warmup','5']
        snapshot(name+'-start')
        with (out/(name+'.csv')).open('w') as csv, (out/(name+'.log')).open('w') as log:
            result=subprocess.run(cmd,stdout=csv,stderr=log)
        snapshot(name+'-end')
        metadata['runs'].append({'name':name,'command':cmd,'exit':result.returncode})
        (out/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
        print(name, result.returncode, flush=True)
        if result.returncode: raise SystemExit(result.returncode)
for name,args in [('mixed_ui',['--frames','360','--warmup','60','--in-flight','3']),('rendering',['--counts','1,1025','--samples','5','--warmup','1']),('paths',['--counts','1,1025','--samples','5','--warmup','1']),('points',['--counts','256,4097','--samples','5'])]:
    cmd=[str(bins/name)]+args
    snapshot(name+'-start')
    with (out/(name+'.csv')).open('w') as csv, (out/(name+'.log')).open('w') as log:
        result=subprocess.run(cmd,stdout=csv,stderr=log)
    snapshot(name+'-end')
    metadata['runs'].append({'name':name,'command':cmd,'exit':result.returncode})
    (out/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
    print(name,result.returncode,flush=True)
    if result.returncode: raise SystemExit(result.returncode)
