import test from 'node:test';import assert from 'node:assert/strict';import {portable,mergePortable} from '../src/profile.js';
test('导出排除设备、短语和路径，导入重新分配稳定引用',()=>{
 const config={endpoint:'private device',phrases:{secret:'private words'},profiles:[{id:'a',name:'默认',apps:['private app'],bindings:{'K2.single':{kind:'sequence',id:'s'}}}],sequences:{s:{name:'组合',steps:[{kind:'text',phrase:'secret'},{kind:'launch',app:'private app'}]}}};
 const data=portable(config);assert.doesNotMatch(JSON.stringify(data),/private|secret/);
 let n=0;const out=mergePortable(config,data,()=>String(++n));assert.equal(out.profiles.length,2);
 const action=out.profiles[1].bindings['K2.single'];assert.equal(out.sequences[action.id].steps[0].kind,'none');
 assert.throws(()=>mergePortable(config,{...data,schemaVersion:42}));assert.equal(config.profiles.length,1);
});
