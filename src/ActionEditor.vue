<script setup>
import ShortcutPicker from './ShortcutPicker.vue';
import AppPicker from './AppPicker.vue';
import targets from './focus-targets.json';
import {ref,watch} from 'vue';
import {invoke} from '@tauri-apps/api/core';
const props=defineProps({action:Object,config:Object,step:Boolean,heading:String,slotKey:String,error:String,status:String});
const emit=defineEmits(['apply','close']);
const isWindows=/Windows/i.test(navigator.userAgent);
const a=props.action??{kind:'none'};
const appName=ref(a.name??'');
const target=ref(a.target??'codex'),tested=ref(false),resetError=ref('');
const editableKind=a=>a.kind==='focusCodex'?'focusChat':a.kind;
watch(target,()=>{tested.value=false;resetError.value=''});
async function restore(){
 try {
  const value=await invoke('default_binding',{slot:props.slotKey});
  kind.value=editableKind(value);target.value=value.target??'codex';keys.value=value.keys?.join('+')??'Enter';steps.value=value.steps??1;
  resetError.value='';
 }catch(e){resetError.value=String(e)}
}
const kind=ref(editableKind(a)), keys=ref(a.keys?.join('+')??'Enter'), steps=ref(a.steps??1), phrase=ref(a.phrase??Object.keys(props.config.phrases)[0]??''), id=ref(a.id??Object.keys(props.config.sequences)[0]??''), app=ref(a.app??''), ms=ref(a.ms??300);
async function tryFocus(){try{resetError.value='';tested.value=true;await invoke('try_focus',{target:target.value})}catch(e){resetError.value=String(e)}}
function apply(){
 const args={none:{},focusChat:{target:target.value},shortcut:{keys:keys.value.split('+').map(s=>s.trim()).filter(Boolean)},scroll:{steps:Number(steps.value)},launch:{app:app.value.trim(),...(appName.value?{name:appName.value}:{})},text:{phrase:phrase.value},sequence:{id:id.value},wait:{ms:Number(ms.value)}};
 emit('apply',{kind:kind.value,...args[kind.value]});
}
</script>
<template><div class="editor"><div class="dialog-head"><small>{{step?'组合动作':'按键映射'}}</small><h2 id="action-title">{{heading}}</h2></div><div class="stack"><label>动作类型<select v-model="kind"><option value="none">未设置</option><option value="shortcut">快捷键</option><option value="scroll">滚动</option><option v-if="isWindows||kind==='focusChat'" value="focusChat">聚焦聊天框</option><option value="launch">打开应用</option><option value="text">常用语</option><option v-if="!step" value="sequence">组合动作</option><option v-if="step" value="wait">等待</option></select></label>
<ShortcutPicker v-if="kind==='shortcut'" v-model="keys"/>
<label v-if="kind==='focusChat'">目标工作台<select v-model="target"><option v-for="item in targets" :key="item.id" :value="item.id">{{item.name}}</option></select></label>
<label v-if="kind==='scroll'">滚动步数 <input v-model.number="steps" type="number" min="-20" max="20"></label>
<AppPicker v-if="kind==='launch'&&isWindows" v-model="app" v-model:name="appName"/>
<label v-else-if="kind==='launch'">程序绝对路径 <input v-model="app" placeholder="/Applications/Example.app"></label>
<select aria-label="选择常用语" v-if="kind==='text'" v-model="phrase"><option v-for="(_,name) in config.phrases" :key="name">{{name}}</option></select>
<select aria-label="选择组合动作" v-if="kind==='sequence'" v-model="id"><option v-for="(s,i) in config.sequences" :key="i" :value="i">{{s.name}}</option></select>
<label v-if="kind==='wait'">毫秒 <input v-model.number="ms" type="number" min="0" max="10000"></label>
<p v-if="kind==='text'&&!Object.keys(config.phrases).length" class="muted">先在「回眸 → 常用语」保存短语。</p>
<p v-if="kind==='sequence'&&!Object.keys(config.sequences).length" class="muted">先在「组合动作」新建一个组合。</p>
<div v-if="!step" class="editor-shortcuts"><button type="button" class="quiet" @click="kind='none'">清除动作</button><button type="button" class="quiet" @click="restore">恢复此手势默认值</button></div><button v-if="kind==='focusChat'" type="button" @click="tryFocus">试聚焦聊天框</button><p v-if="kind==='focusChat'" class="muted" role="status">{{tested && status ? status : '聚焦 Windows 桌面工作台的当前对话后，可按住 AU05 语音键说话。'}}</p><p v-if="error||resetError" role="alert" class="pageError">{{error||resetError}}</p>
<div class="dialog-actions"><button @click="emit('close')">取消</button><button class="primary" :disabled="(kind==='shortcut'&&!keys.trim())||(kind==='launch'&&!app.trim())||(kind==='text'&&!phrase)||(kind==='sequence'&&!id)" @click="apply">保存动作</button></div></div></div></template>
