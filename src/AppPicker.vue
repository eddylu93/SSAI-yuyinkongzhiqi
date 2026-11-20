<script setup>
import {computed,onMounted,ref} from 'vue';
import {invoke} from '@tauri-apps/api/core';

const app=defineModel({type:String,required:true});
const name=defineModel('name',{type:String,default:''});
const apps=ref([]),busy=ref(false),error=ref('');
const manual=ref(!!app.value&&!name.value);
const priority=/^(codex|chatgpt|claude|deepseek|workbuddy|微信$|wechat$|weixin$|豆包|doubao)/i;
const common=computed(()=>apps.value.filter(item=>priority.test(item.name)));
const other=computed(()=>apps.value.filter(item=>!priority.test(item.name)));
const missing=computed(()=>!!app.value&&!manual.value&&!apps.value.some(item=>item.app===app.value));

async function refresh(){
  if(busy.value)return;
  busy.value=true;error.value='';
  try { apps.value=await invoke('installed_apps'); }
  catch(e){error.value=String(e)}
  finally {busy.value=false}
}
function choose(value){
  manual.value=value==='manual';
  const item=apps.value.find(candidate=>candidate.app===value);
  app.value=item?.app??'';name.value=item?.name??'';
}
onMounted(refresh);
</script>
<template>
  <div class="app-picker">
    <label>本机应用
      <select aria-label="本机应用" :value="manual?'manual':app" @change="choose($event.target.value)">
        <option value="" disabled>{{busy?'正在读取应用…':'选择已安装的应用'}}</option>
        <option v-if="missing" :value="app">{{name||app}}{{busy?'':'（当前列表中未找到）'}}</option>
        <optgroup v-if="common.length" label="常用应用"><option v-for="item in common" :key="item.app" :value="item.app" :title="item.app">{{item.name}}</option></optgroup>
        <optgroup v-if="other.length" label="其他应用"><option v-for="item in other" :key="item.app" :value="item.app" :title="item.app">{{item.name}}</option></optgroup>
        <option value="manual">手动输入路径…</option>
      </select>
    </label>
    <button type="button" :disabled="busy" @click="refresh" aria-label="刷新本机应用列表">刷新</button>
  </div>
  <p v-if="error" role="alert" class="pageError">{{error}}</p>
  <small v-else-if="!busy&&!apps.length" role="status">未找到已注册的桌面应用，可手动输入路径。</small>
  <label v-if="manual">程序绝对路径 <input v-model="app" placeholder="C:\Windows\notepad.exe"></label>
</template>
