export function portable(c){
  const sanitize=a=>['launch','text'].includes(a.kind)?{kind:'none'}:structuredClone(a);
  return {schemaVersion:1,model:'AU05',profiles:c.profiles.map(p=>({id:p.id,name:p.name,apps:[],bindings:Object.fromEntries(Object.entries(p.bindings).map(([k,a])=>[k,sanitize(a)]))})),
    sequences:Object.fromEntries(Object.entries(c.sequences).map(([id,s])=>[id,{name:s.name,steps:s.steps.map(sanitize)}]))};
}
export function mergePortable(config,file,makeId=()=>crypto.randomUUID()){
  if(file.schemaVersion!==1||file.model!=='AU05'||!Array.isArray(file.profiles)||!file.profiles.length)throw Error('文件版本或设备型号不匹配');
  const out=structuredClone(config), ids=new Map(), seen=new Set();
  for(const id of Object.keys(file.sequences??{})) ids.set(id,makeId());
  function action(a){
    if(!a||!['none','focusCodex','focusChat','shortcut','scroll','wait','sequence'].includes(a.kind))throw Error('文件包含不可移植的动作');
    if(a.kind==='sequence'){if(!ids.has(a.id))throw Error('组合引用缺失');return {kind:'sequence',id:ids.get(a.id)}}
    return structuredClone(a);
  }
  for(const [id,s] of Object.entries(file.sequences??{}))out.sequences[ids.get(id)]={name:s.name,steps:s.steps.map(action)};
  for(const p of file.profiles){
    if(!p.name||!p.id||seen.has(p.id))throw Error('配置名称或标识无效');seen.add(p.id);
    out.profiles.push({id:makeId(),name:p.name+'（导入）',apps:[],bindings:Object.fromEntries(Object.entries(p.bindings).map(([k,a])=>[k,action(a)]))});
  }
  return out;
}
