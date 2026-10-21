import test from "node:test";
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {Spectrum,Wavelet,WaveletMap} from "../node.js";
import init,{Wavelet as BrowserWavelet,WaveletMap as BrowserMap} from "../browser.js";
const reference=JSON.parse(await readFile(new URL("../../crates/rexafs/tests/fixtures/analysis/wavelet/larch-reference.json",import.meta.url)));
const close=(a,b,tolerance)=>assert.ok(Math.abs(a-b)<tolerance,`${a} versus ${b}`);
function options(c) {return {kweight:c.kweight,order:c.order,kstep:c.kstep,nfft:c.actual_fft_length,radii:c.r};}

test("native wavelet reference, row-major arrays, ownership and saved replay",()=>{
  for (const c of reference.cases) {
    const k=Float64Array.from(c.k),chi=Float64Array.from(c.chi);
    const model=new Wavelet([k[0],k.at(-1)],options(c)),before=model.to_json();
    const map=model.calculate(k,chi);
    assert.deepEqual(map.shape,[c.r.length,k.length]);
    for (const name of ["real","imaginary"]) map[name].forEach((v,i)=>close(v,c[name][i],reference.absolute_tolerance));
    assert.equal(model.to_json(),before);assert.equal(map.preparation,null);
    assert.equal(model.estimate(k).cells,map.real.length);
    const region=map.integral([1,3],[.2,1]),saved=map.to_json();assert.ok(region.value>0);
    assert.equal(region.method,"bilinear_magnitude_v1");
    const mean=map.mean([1,3],[.2,1]),maximum=map.maximum([1,3],[.2,1]);
    close(mean.value,region.value/1.6,1e-12);assert.ok(maximum.value>=mean.value);
    assert.equal(mean.method,"bilinear_magnitude_mean_v1");
    assert.equal(maximum.method,"bilinear_magnitude_maximum_v1");
    for(const method of ["mean","maximum"]) assert.throws(()=>map[method]([0,100],[.2,1]));
    const copied=map.real;copied.fill(999);chi.fill(999);
    assert.deepEqual(Array.from(map.input_chi),c.chi);assert.equal(map.to_json(),saved);
    const restored=WaveletMap.from_json(saved),definition=map.definition,replay=definition.calculate(map.input_k,map.input_chi);
    assert.deepEqual(restored.real,map.real);assert.deepEqual(replay.real,map.real);
    assert.equal(restored.integral([1,3],[.2,1]).value,region.value);
    const nk=map.shape[1],magnitude=map.magnitude;
    assert.deepEqual(map.slice_at_r(map.r[2]),magnitude.slice(2*nk,3*nk));
    const column=map.slice_at_k(map.k[2]);column.forEach((v,i)=>assert.equal(v,magnitude[i*nk+2]));
    map.phase();assert.equal(map.to_json(),saved);
    const retained=map.real;map.free();assert.equal(retained.length,c.real.length);assert.throws(()=>map.real);
    for (const object of [model,restored,definition,replay]) object.free();
  }
});

test("spectrum wavelet prepares on a copy and matches explicit processing",()=>{
  const energy=Float64Array.from({length:1421},(_,i)=>8760+i);
  const mu=energy.map(e=>{const k=Math.sqrt(Math.max(0,e-8980)/3.80998212);return .1+.00003*(e-8980)+(1+.06*Math.sin(4.6*k)*Math.exp(-(((k-8)/4)**2)))/(1+Math.exp(-(e-8980)/1.5));});
  const source=new Spectrum(energy,mu),model=new Wavelet([2,12]),map=source.wavelet(model);
  assert.equal(source.norm(),undefined);assert.equal(source.chi(),undefined);
  assert.equal(map.preparation.backend,"nalgebra");
  source.calc_background();const replay=model.calculate(source.k(),source.chi());assert.deepEqual(map.real,replay.real);
  for (const object of [source,model,map,replay]) object.free();
});

test("browser initialization, phase mask, invalid coverage and malformed maps",async()=>{
  assert.throws(()=>new BrowserWavelet([1,9]),/init/);
  await init(await readFile(new URL("../dist/web/rexafs_wasm_bg.wasm",import.meta.url)));
  const k=Float64Array.from({length:101},(_,i)=>i*.1),model=new BrowserWavelet([1,9]),map=model.calculate(k,new Float64Array(k.length));
  assert.ok(map.phase().every(Number.isNaN));assert.equal(map.integral([2,8],[1,3]).value,0);
  assert.equal(map.mean([2,8],[1,3]).value,0);assert.equal(map.maximum([2,8],[1,3]).value,0);
  assert.throws(()=>map.phase(2));assert.throws(()=>map.integral([0,3],[1,3]));
  assert.throws(()=>new BrowserWavelet([1,9],{typo:2}),/Unknown/);
  assert.throws(()=>new BrowserWavelet([1,9],{rmax:NaN}),/finite/);
  const partial=new BrowserWavelet([0,20]),outside=new BrowserWavelet([12,20]),huge=new BrowserWavelet([1,9],{kstep:1e-100});
  const limited=partial.calculate(k,k);assert.match(limited.warnings_json(),/limited to the measured data/);
  assert.throws(()=>outside.calculate(k,k),/outside the measured k range/);assert.throws(()=>huge.estimate(k));
  const bad=JSON.parse(map.to_json());bad.real.pop();assert.throws(()=>BrowserMap.from_json(JSON.stringify(bad)),/dimensions/);
  const restored=BrowserMap.from_json(map.to_json());assert.deepEqual(restored.real,map.real);
  for(const object of [model,map,partial,limited,outside,huge,restored]) object.free();
});
