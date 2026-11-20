<script setup>
import {computed,ref,onMounted,onBeforeUnmount} from 'vue';
const value=defineModel({type:String,required:true});
const expanded=ref(false), recording=ref(false), root=ref(null), message=ref('');
const mac=/Macintosh/.test(navigator.userAgent);
const modifiers=['Ctrl','Win','Alt','Shift','RightShift','RightAlt','RightWin','RightCtrl'];
const move=['Home','End','PageUp','PageDown','Left','Up','Down','Right','Delete'];
const rows=[['Esc',...Array.from({length:12},(_,i)=>'F'+(i+1))],
 [...'1234567890','Backspace'],['Tab',...'QWERTYUIOP'],[...'ASDFGHJKL','Enter'],
 [...'ZXCVBNM','Space'],modifiers,move];
const captions={Left:'←',Right:'→',Up:'↑',Down:'↓',PageUp:'PgUp',PageDown:'PgDn',RightCtrl:'右 Ctrl',
 RightAlt:mac?'右 Option':'右 Alt',RightShift:'右 Shift',RightWin:mac?'右 ⌘':'右 Win',Win:mac?'⌘':'Win'};
const names={ControlLeft:'LeftCtrl',ControlRight:'RightCtrl',ShiftLeft:'LeftShift',ShiftRight:'RightShift',
 AltLeft:'LeftAlt',AltRight:'RightAlt',MetaLeft:'Win',MetaRight:'RightWin',Escape:'Esc',ArrowLeft:'Left',
 ArrowRight:'Right',ArrowUp:'Up',ArrowDown:'Down',Space:'Space',AudioVolumeMute:'Mute',AudioVolumeUp:'VolumeUp',
 AudioVolumeDown:'VolumeDown',MediaPlayPause:'PlayPause'};
const aliases={LeftCtrl:'Ctrl',LeftAlt:'Alt',LeftShift:'Shift',LeftCmd:'Win',Cmd:'Win',RightCmd:'RightWin',Option:'Alt',LeftOption:'Alt',RightOption:'RightAlt'};
const selected=computed(()=>value.value.split('+').map(k=>k.trim()).filter(Boolean));
function modifier(k){return modifiers.includes(aliases[k]??k)}
function pick(key){
 message.value='';
 const mods=selected.value.filter(modifier);
 if(modifier(key)){
  const retained=selected.value.filter(k=>(aliases[k]??k)!==key);
  const next=retained.length===selected.value.length?[...mods,key,...selected.value.filter(k=>!modifier(k))]:retained;
  if(next.length>5){message.value='每组快捷键最多选择五个键';return}
  value.value=next.join('+');
 }else if(mods.length<5){value.value=[...mods,key].join('+')}else{message.value='请先取消一个修饰键'}
}
const down=new Set();let pending=[],consumed=new Set(),dialog;
function start(){recording.value=!recording.value;down.clear();pending=[];consumed.clear();message.value=''}
function stop(){recording.value=false;down.clear();pending=[]}
function capture(e){
 if(!recording.value&&!consumed.has(e.code))return;
 e.preventDefault();e.stopPropagation();
 if(e.repeat||!recording.value)return;
 consumed.add(e.code);
 const key=names[e.code]??(/^(Key[A-Z]|Digit\d)$/.test(e.code)?e.code.replace(/^(Key|Digit)/,''):e.code);
 const isMod=modifier(key);
 if(!isMod&&!rows.flat().includes(key)&&!/^F(?:1[3-9]|2[0-4])$/.test(key)&&!['Mute','VolumeUp','VolumeDown','PlayPause'].includes(key)){
   message.value='当前快捷键不支持这个键，请重新录入';stop();return;
 }
 if(isMod){down.add(key);pending=[...down];return}
 const mods=[...down];
 for(const [pressed,generic] of [[e.ctrlKey,'Ctrl'],[e.altKey,'Alt'],[e.shiftKey,'Shift'],[e.metaKey,'Win']]){
   if(pressed&&!mods.some(k=>(aliases[k]??k).replace(/^Right/,'')===generic))mods.push(generic);
 }
 if(mods.length<5){value.value=[...mods,key].join('+')}else{message.value='每组快捷键最多录入五个键'}
 stop();
}
function release(e){
 if(!consumed.delete(e.code))return;
 e.preventDefault();e.stopPropagation();
 if(recording.value&&pending.length){
   down.delete(names[e.code]);
   if(!down.size){if(pending.length<=5)value.value=pending.join('+');else message.value='每组快捷键最多录入五个键';stop()}
 }
}
function cancel(e){
 // WebView2 can deliver a native dialog cancel after the keyup has settled.
 // While editing a shortcut, the explicit Cancel button closes the dialog.
 e.preventDefault();
 if(recording.value){
   if(down.size<5)value.value=[...down,'Esc'].join('+');
   else message.value='每组快捷键最多录入五个键';
   stop();
 }
}
onMounted(()=>{
 dialog=root.value.closest('dialog');
 dialog?.setAttribute('closedby','none');
 dialog?.addEventListener('cancel',cancel);
});
onBeforeUnmount(()=>{
 dialog?.removeEventListener('cancel',cancel);
 dialog?.removeAttribute('closedby');
});
</script>
<template><div ref="root" class="shortcut-picker stack">
<label>键值<input v-model="value" placeholder="Ctrl+Shift+V" @keydown.esc.prevent.stop="value=[$event.ctrlKey&&'Ctrl',$event.altKey&&'Alt',$event.shiftKey&&'Shift',$event.metaKey&&'Win','Esc'].filter(Boolean).join('+')" @keyup.esc.prevent.stop></label>
<div class="picker-tools"><button type="button" :aria-pressed="recording" @click="start" @keydown="capture" @keyup="release" @blur="stop();consumed.clear()">{{recording?'请按下快捷键…':'录入快捷键'}}</button>
<button type="button" :aria-expanded="expanded" @click="expanded=!expanded">虚拟键盘</button></div>
<div v-if="expanded" class="key-board" role="group" aria-label="虚拟键盘">
<div v-for="(row,index) in rows" :key="index" class="key-row"><button v-for="key in row" :key="key" type="button" :aria-label="key" :aria-pressed="selected.some(k=>(aliases[k]??k)===key)" @click="pick(key)">{{captions[key]??key}}</button></div>
<div class="board-foot"><small>点击修饰键可选中或取消，再选择主键。</small><button type="button" class="quiet" @click="value='';message=''">清空</button></div></div>
<small v-if="message" role="status">{{message}}</small></div></template>
