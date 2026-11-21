<script setup>
import {computed,ref} from 'vue';
import BatteryReading from './BatteryReading.vue';
import unit from '../docs/界面素材/AU05.png';
import {invoke} from '@tauri-apps/api/core';
const props=defineProps({state:Object,config:Object,section:String});
const emit=defineEmits(['changed','error','control']);
const role=ref('K1'),message=ref(''),busy=ref(false);
const roles={K1:'语音键',K2:'确认键',K3:'取消键',DIAL:'旋钮按下',left:'旋钮左旋',right:'旋钮右旋',heldLeft:'按住左旋',heldRight:'按住右旋'};
const hid=computed(()=>props.state.direct??{}),learn=computed(()=>hid.value.learning);
const instruction=computed(()=>['K1','K2','K3','DIAL'].includes(role.value)?'按住所选控件一秒，再松开。':'沿所选方向转动一格。');
async function command(action){
 busy.value=true;message.value='';
 try{message.value=await invoke('hid_calibration',{command:action,role:role.value});emit('changed',await invoke('state'))}
 catch(e){emit('error',String(e));emit('changed',await invoke('state'))}
 finally{busy.value=false}
}
async function test(value){
 try{await invoke('diagnostic',{value});emit('changed',await invoke('state'))}catch(e){emit('error',String(e))}
}
</script>
<template>
 <template v-if="section==='设备连接'">
  <div class="receiver block">
    <div class="receiver-photo"><img :src="unit" alt="AU05 机身正面"></div>
    <div><h2>Ulanzi AU05</h2><p><i class="dot" :class="{online:state.online}"></i>{{state.online?'USB 接收器已连接':'等待连接 USB 接收器'}}</p>
    <dl class="device-facts"><div><dt>机身</dt><dd>{{hid.transmitter==null?'状态未知':hid.transmitter?'在线':'离线'}}</dd></div><div><dt>电量</dt><dd><BatteryReading :state="state" detailed/></dd></div><div><dt>输入接口</dt><dd>{{hid.sources?.length??0}} 个</dd></div></dl></div>
  </div>
  <div class="block"><div class="setting-row"><div><h2>设备直控</h2><p class="muted">通过 USB 接收器读取三个实体键和旋钮。</p></div>
    <label><input class="switch" aria-label="启用设备直控" type="checkbox" :checked="config.vendorControls" @change="emit('control',$event.target.checked)"></label></div>
    <p class="state-line"><i class="dot" :class="{online:hid.vendorActive}"></i>{{hid.vendorStatus||'正在打开设备接口'}}</p>
    <details><summary>接口与事件详情</summary><p>{{hid.status}}</p><p>{{config.vendorControls?'直控事件 '+(hid.vendorEvents??0)+' 次':'标准输入报告 '+(hid.reports??0)+' 份'}}<span v-if="!config.vendorControls&&hid.lastSource"> · {{hid.lastSource}}</span></p><ul><li v-for="source in hid.sources" :key="source">{{source}}</li></ul></details>
  </div></template>
 <div v-if="section==='按键测试'" class="block">
  <h2>{{config.vendorControls?'实体按键测试':'实体控件校准'}}</h2>
  <template v-if="!config.vendorControls">
  <p>选择一个控件，重复两次相同操作。校准期间只观察 AU05 的输入。其他应用可能同时收到设备自带的键盘或滚轮操作。</p>
  <div class="row"><select aria-label="选择校准控件" v-model="role" :disabled="!!learn"><option v-for="(name,id) in roles" :value="id" :key="id">{{name}}{{hid.calibrated?.includes(id)?'（已校准）':''}}</option></select>
  <button class="primary" v-if="!learn" :disabled="!state.online||busy" @click="command('begin')">开始校准</button>
  <template v-else><button class="primary" :disabled="busy||!learn.received" @click="command('finish')">本次完成</button><button @click="command('cancel')">取消</button></template></div>
  <p v-if="learn">第 {{learn.round}} 次 · {{instruction}} {{learn.received?'已收到报告。':'等待设备输入…'}}</p><p v-if="message" role="status">{{message}}</p></template>
  <p v-else>勾选测试按键，按住语音键再松开，检查 K1 的 down 和 up。确认和取消分别对应 K2、K3。</p>
  <label><input type="checkbox" :checked="state.diagnostic" :disabled="!!learn" @change="test($event.target.checked)">测试按键（仅显示事件）</label>
  <template v-if="state.diagnostic && state.inputTest">
    <p>完整按下并松开<span v-for="key in ['K1','K2','K3','DIAL']" :key="key">　{{roles[key]}} {{state.inputTest.pairs[key]??0}} 次</span></p>
    <p>旋钮步数<span v-for="key in ['left','right','heldLeft','heldRight']" :key="key">　{{roles[key]}} {{state.inputTest.turns[key]??0}}</span></p>
  </template>
  <p class="muted">{{state.recent?.slice(0,5).join('　')}}</p>
 </div>
</template>
