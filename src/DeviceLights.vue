<script setup>
import {ref,watch,onBeforeUnmount} from 'vue';
import {invoke} from '@tauri-apps/api/core';

const props=defineProps({available:Boolean});
const state=ref(null),busy=ref(false),message=ref(''),error=ref('');
const modes=['全灭','全亮','工作模式'];
const levels=['灭','低','高'];
const lamps=['语音键 K1','确认键 K2','取消键 K3','旋钮'];
const effects=['灭','常亮','呼吸'];
let generation=0;
async function sync(change=null) {
  const version=++generation;
  busy.value=true; error.value=''; message.value='';
  try {
    const reply=await invoke('lighting',{change});
    if(version!==generation)return;
    state.value=reply;
    message.value=change?'设备已确认灯光设置':'已读取设备设置';
  } catch(e) {
    if(version!==generation)return;
    state.value=null; error.value=String(e);
  } finally {if(version===generation)busy.value=false;}
}
watch(()=>props.available,available=>{
  generation++;state.value=null;busy.value=false;error.value='';message.value='';
  if(available)sync();
},{immediate:true});
onBeforeUnmount(()=>generation++);
</script>
<template>
  <div class="block"><div class="setting-row"><h2>机身灯光</h2><button :disabled="!available||busy" @click="sync()">重新读取</button></div>
    <p v-if="!available">连接接收器并唤醒 AU05 后可读取灯光设置。</p>
    <p v-if="error" role="alert" class="err">{{error}}</p>
    <template v-if="state">
      <div class="setting-row"><div><strong>灯光模式</strong><p class="muted">工作模式可分别设置按键和旋钮的指示灯。</p></div>
        <div class="segments" role="group" aria-label="灯光模式"><button v-for="(name,value) in modes" :key="name" :aria-pressed="state.mode===value" :disabled="busy" @click="sync({kind:'mode',value})">{{name}}</button></div>
      </div>
      <div v-if="state.mode===1" class="setting-row"><strong>全亮亮度</strong>
        <div class="segments" role="group" aria-label="全亮亮度"><button v-for="(name,value) in levels" :key="name" :aria-pressed="state.brightness===value" :disabled="busy" @click="sync({kind:'brightness',value})">{{name}}</button></div>
      </div>
      <template v-if="state.mode===2"><div v-for="(name,index) in lamps" :key="name" class="setting-row"><strong>{{name}}</strong>
        <select :aria-label="name+'灯效'" :disabled="busy" :value="state.lamps[index]" @change="sync({kind:'lamp',index,value:Number($event.target.value)})">
          <option v-for="(effect,value) in effects" :key="effect" :value="value">{{effect}}</option>
        </select></div><p class="muted">呼吸灯的持续时间取决于机身固件。</p></template>
    </template>
    <p class="muted" role="status">{{busy?'正在与接收器通信…':message}}</p>
  </div>
</template>
