#!/usr/bin/env python3
"""Verify/map/route every successful bounded UFO-MAC control, serially."""
import csv
import json
from pathlib import Path
import re
import subprocess
from compare_mac_arrival import ENV, HELDOUT, LIB, FLOW, run, sha, write_tsv, dominates
from prove_mac_mapped import prove_pair

ROOT = Path('results/discovery/mac_ufomac_challenge_20260905')


def main():
    p = json.loads((ROOT / 'protocol.json').read_text())
    assert sha(LIB) == p['liberty_sha256'] and sha(FLOW) == p['flow_sha256']
    assert (ROOT / 'optimized_graph.json').is_file()
    assert not (ROOT / 'rtl').exists(), 'preserve previous results'
    (ROOT / 'evaluation_sources.json').write_text(json.dumps({str(f): sha(f) for f in [
        Path(__file__), Path('src/mac.rs'), Path('src/mac_import.rs'),
        Path('examples/mac_import_ufomac.rs'), ROOT/'optimized_graph.json']}, indent=2)+'\n')
    run(['nice', '-n', '19', 'cargo', 'run', '--locked', '--release', '--example',
         'mac_import_ufomac', '--', str(HELDOUT/'genome.json'),
         str(ROOT/'optimized_graph.json'), str(ROOT/'rtl')], ROOT/'generation.log', 180)
    sources = sorted((ROOT/'rtl').glob('*.v'))
    assert len(sources) == 6
    run(['nice', '-n', '19', 'python3', 'scripts/prove_mac_composed.py', '--cells',
         str(HELDOUT/'cells.v'), '--output', str(ROOT/'formal'),
         *map(str, sources)], ROOT/'formal.log', 240)
    print('Six controls passed simulation and composed proof; frozen candidate byte-identical', flush=True)
    mapped = ROOT/'mapped'
    mapped.mkdir()
    rows, proofs = [], []
    for src in sources:
        for recipe in ('default', 'classic'):
            name = src.stem+'_'+recipe
            arg = '' if recipe == 'default' else '-script scripts/abc/classic.script'
            script = f'read_verilog {HELDOUT}/cells.v {src}; attrmap -modattr -remove keep_hierarchy=1; hierarchy -top {src.stem}; proc; flatten; opt; techmap; opt; abc -liberty {LIB} {arg}; opt_clean; rename {src.stem} {name}; write_verilog -noattr -noexpr {mapped}/{name}.v'
            (ROOT/f'{name}.ys').write_text(script+'\n')
            run(['nice','-n','19','yosys','-Q','-s',str(ROOT/f'{name}.ys')],ROOT/f'{name}.log',60)
            proof = prove_pair(src,mapped/f'{name}.v',HELDOUT/'cells.v',LIB,ROOT/'mapped_proof'/name,24)
            proof['module'] = name
            proofs.append(proof)
            (ROOT/'mapped_proof.json').write_text(json.dumps(proofs,indent=2)+'\n')
            assert proof['status']=='proved'
            rows.append({'module':name,'recipe':recipe,'class':'ufomac_bounded',
                         'source_sha256':sha(src),'mapped_sha256':sha(mapped/f'{name}.v')})
            print(f'Mapped/proved {len(rows)}/12 {name}',flush=True)
    run(['docker','run','--rm','--cpus','2','--platform','linux/amd64','-v',f'{Path.cwd()}:/work',
         '-w','/work','-e','OMP_NUM_THREADS=2','-e',f'LIB={LIB}','-e',f'MAPPED={mapped}',
         '-e',f'OUT={ROOT}/sta.tsv',p['image_id'],
         '/OpenROAD-flow-scripts/tools/install/OpenROAD/bin/sta','-exit','scripts/sta_cores.tcl'],ROOT/'sta.log',90)
    timing = {r['module']:r for r in csv.DictReader((ROOT/'sta.tsv').open(),delimiter='\t')}
    assert len(timing)==12
    rows = [{**r,**timing[r['module']]} for r in rows]
    write_tsv(ROOT/'mapped_summary.tsv',rows)
    routed=[]
    for i,row in enumerate(rows):
        name=row['module']; prefix=ROOT/'route'/name;prefix.parent.mkdir(exist_ok=True)
        container=f'macufomac-{i}'
        try:
            run(['docker','run','--name',container,'--rm','--cpus','2','--platform','linux/amd64',
                 '-v',f'{Path.cwd()}:/work','-w','/work','-e','OMP_NUM_THREADS=2',
                 '-e',f'NETLIST={mapped}/{name}.v','-e',f'TOP={name}','-e',f'OUT={prefix}',
                 '-e','UTILIZATION=45',p['image_id'],
                 '/OpenROAD-flow-scripts/tools/install/OpenROAD/bin/openroad','-threads','2',
                 '-exit',str(FLOW)],prefix.with_suffix('.log'),120)
        finally:
            subprocess.run(['docker','rm','-f',container],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=False)
        log=prefix.with_suffix('.log').read_text();report=prefix.with_suffix('.checks.rpt').read_text()
        routed.append({**row,'worst_arrival_ps':f'{float(re.search(r"([0-9.]+)\s+data arrival time",report)[1])*1000:.1f}',
                       'area_um2':re.findall(r'^Design area\s+([0-9.]+)',log,re.M)[-1],
                       'hpwl_um':re.findall(r'^legalized HPWL\s+([0-9.]+)',log,re.M)[-1]})
        write_tsv(ROOT/'routed_summary.tsv',routed)
        print(f'Routed {i+1}/12 {name}: {routed[-1]["worst_arrival_ps"]} ps / {routed[-1]["area_um2"]} um2',flush=True)
    old=list(csv.DictReader(Path('results/discovery/mac_arrival_challenge_20260905/routed_summary.tsv').open(),delimiter='\t'))
    for recipe in ('default','classic'):
        candidate=next(r for r in old if r['recipe']==recipe and r['class']=='candidate')
        print(recipe,'candidate dominators:',[r['module'] for r in routed if r['recipe']==recipe and dominates(r,candidate)],flush=True)


if __name__=='__main__':
    main()
