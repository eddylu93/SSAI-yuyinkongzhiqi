<script setup>
import {ref,computed,onMounted,onBeforeUnmount,nextTick,watch} from 'vue';
import {invoke} from '@tauri-apps/api/core';
import ProjectInfo from './ProjectInfo.vue';
import rawRepository from './repository.txt?raw';
import {checkRelease} from './releases.js';
import {version} from '../package.json';
import ShortcutPicker from './ShortcutPicker.vue';
import StatsPanel from './StatsPanel.vue';
import {groups,gestures} from './mappings.js';
import HistoryPanel from './HistoryPanel.vue';
import DeviceLights from './DeviceLights.vue';
import DeviceConnection from './DeviceConnection.vue';
import Mapping from './Mapping.vue';import ActionEditor from './ActionEditor.vue';import {describe} from './mappings.js';import {portable,mergePortable} from './profile.js';import './style.css';
import {PhKeyboard,PhStack,PhChartBar,PhChatText,PhLink,PhShield,PhInfo,PhExport,PhCaretDown,PhSlidersHorizontal,PhCheckCircle,PhWarningCircle,PhMicrophone,PhCaretRight} from '@phosphor-icons/vue';
const nav=[['按键',PhKeyboard],['组合动作',PhStack],['统计',PhChartBar],['回眸',PhChatText],['连接',PhLink],['权限',PhShield],['关于',PhInfo],['分享',PhExport]];
function windowAction(action){return act(()=>invoke('window_action',{action}))}
const tab=ref('按键'),c=ref(),s=ref({}),error=ref(''),notice=ref(''),devices=ref([]),audio=ref(''),busy=ref(false),slot=ref(''),dialog=ref(),seqId=ref(''),stepIndex=ref(null),span=ref('全部'),counts=ref([]),rank=ref([]),seqName=ref(''),newProfile=ref(''),importFile=ref(),agentName=ref(''),grants=ref([]),snippet=ref('');
const profile=computed(()=>c.value?.profiles.find(p=>p.id===c.value.active)),seq=computed(()=>c.value?.sequences[seqId.value]);
const connected=computed(()=>s.value.online===true && s.value.direct?.transmitter===true);
const battery=computed(()=>{
 const level=s.value.direct?.battery;
 return connected.value&&Number.isInteger(level)&&level>=0&&level<=100?level:null;
});
const systemTheme=window.matchMedia('(prefers-color-scheme: light)');
const prefersLight=ref(systemTheme.matches);
function syncTheme(event){prefersLight.value=event.matches}
const sections={'连接':['设备连接','语音输入','按键测试','灯光'],'回眸':['听写记录','常用语','客户端授权'],'关于':['应用设置','按键配置']};
const sub=ref(''),content=ref(null);
function selectSection(value){sub.value=value;content.value?.scrollTo(0,0)}
const titles={'按键':'按键映射','连接':'设备连接','回眸':'回眸','组合动作':'组合动作','统计':'使用统计','权限':'权限与诊断','关于':'关于与设置','分享':'方案分享'};
const editorTitle=computed(()=>{
 if(stepIndex.value!==null)return '组合步骤 '+(stepIndex.value+1);
 const [key,gesture]=slot.value.split('.');
 const group=groups.find(g=>g.control===key&&g.gestures.includes(gesture));
 if(!group)return '编辑动作';
 return group.gestures.length===1?group.name:group.name+' · '+gestures[gesture];
});

const testText=ref(''),confirmedText=ref(false);
const setupSteps=computed(()=>[
 {label:'接收器直控',done:!!s.value.direct?.vendorActive},
 {label:'确认键按下并松开',done:(s.value.inputTest?.pairs?.K2??0)>0},
 {label:'选择麦克风和语音工具',done:!!c.value?.voiceEnabled&&devices.value.includes(c.value?.endpoint)},
 {label:'在输入框确认出字',done:confirmedText.value}
]);
async function finishSetup(skip){
 if(!skip&&!setupSteps.value.every(step=>step.done))return;
 const saved=await invoke('save',{value:{...JSON.parse(JSON.stringify(c.value)),setupComplete:true}});
 c.value=saved;await invoke('diagnostic',{value:false});s.value=await invoke('state');await go('按键');
}
async function startSetup(){
 c.value.setupComplete=false;if(!await save()){c.value.setupComplete=true;return}
 await go('连接');await invoke('diagnostic',{value:true});s.value=await invoke('state');confirmedText.value=false;testText.value='';
}
const editorSession=ref(0), loginEnabled=ref(false);
async function login(value){loginEnabled.value=await invoke('login_start',{enabled:value})}
function diagnosis(){
 const state=s.value,device=state.direct??{};
 return ['SSAI-语音控制器 '+version+' · 系统 HID','接收器：'+(state.online?'就绪':'离线'),
  '机身：'+(device.transmitter==null?'未知':device.transmitter?'在线':'离线'),
  '设备直控：'+(device.vendorActive?'就绪':c.value?.vendorControls?'等待确认':'关闭'),
  '输入接口：'+(device.sources?.length??0),'校准控件：'+(device.calibrated?.length??0),
  '直控事件：'+(device.vendorEvents??0),'输入报告：'+(device.reports??0),
  '测试按键：'+(state.diagnostic?'开启':'关闭'),'收到事件：'+(state.events??0)].join('\n');
}
async function copy(text){await navigator.clipboard.writeText(text);notice.value='已复制'}
function bindingCount(id){return c.value.profiles.reduce((total,p)=>total+Object.values(p.bindings).filter(a=>a.kind==='sequence'&&a.id===id).length,0)}
async function duplicateSeq(){const other=JSON.parse(JSON.stringify(seq.value));other.name+='（副本）';const id=crypto.randomUUID();c.value.sequences[id]=other;seqId.value=id;await save()}
async function removeSeq(){const id=seqId.value;if(!confirm('删除此组合，并清除 '+bindingCount(id)+' 个按键绑定？'))return;
 for(const p of c.value.profiles)for(const [slot,a] of Object.entries(p.bindings))if(a.kind==='sequence'&&a.id===id)delete p.bindings[slot];
 delete c.value.sequences[id];seqId.value='';await save()}
async function removeProfile(){if(c.value.active==='default'||!confirm('删除当前配置？'))return;c.value.profiles=c.value.profiles.filter(p=>p.id!==c.value.active);c.value.active='default';await save()}
let timer,noticeTimer,alive=true;
watch(notice,value=>{clearTimeout(noticeTimer);if(value)noticeTimer=setTimeout(()=>{notice.value=''},3500)});
async function act(fn){try{error.value='';return await fn()}catch(e){error.value=String(e);return undefined}}
async function load(){c.value=await invoke('config');s.value=await invoke('state')}
async function save(){const result=await act(()=>invoke('save',{value:JSON.parse(JSON.stringify(c.value))}));if(result){c.value=result;notice.value='已保存';return true}return false}
async function go(t,section){if(tab.value==='连接' && t!=='连接')audio.value='';
 tab.value=t;selectSection(section??sections[t]?.[0]??'');notice.value='';error.value='';if(t==='统计')await totals();if(t==='回眸'){await fetchGrants()}if(t==='连接')await detect();if(t==='关于')await login(null)}
async function setVendor(value){
 const previous=c.value.vendorControls;c.value.vendorControls=value;
 if(!await save())c.value.vendorControls=previous;
}
async function detect(){devices.value=await invoke('devices')}
async function listen(){audio.value='';busy.value=true;try{const result=await invoke('sample',{name:c.value.endpoint});if(tab.value==='连接')audio.value=result}finally{busy.value=false}}
let statsGeneration=0;
async function totals(){const request=++statsGeneration;const d=new Date();d.setHours(0,0,0,0);if(span.value==='本周')d.setDate(d.getDate()-((d.getDay()+6)%7));const since=span.value==='全部'?0:Math.floor(+d/1000);const result=await Promise.all([invoke('stats',{since}),invoke('ranking',{since})]);if(request===statsGeneration)[counts.value,rank.value]=result}
async function fetchGrants(){grants.value=await invoke('clients')}
async function grant(){snippet.value=await invoke('grant',{name:agentName.value});await fetchGrants()}
const voiceDialog=ref(null),voiceDraft=ref(''),editingVoice=ref(false);
async function editVoiceShortcut(){
 error.value='';voiceDraft.value=c.value.voiceKeys.join('+');editingVoice.value=true;
 await nextTick();voiceDialog.value.showModal();
}
async function saveVoiceShortcut(){
 const value=JSON.parse(JSON.stringify(c.value));
 value.voiceKeys=voiceDraft.value.split('+').map(k=>k.trim()).filter(Boolean);
 const saved=await act(()=>invoke('save',{value}));
 if(saved){c.value=saved;notice.value='已保存';voiceDialog.value.close()}
}
async function editor(key,index=null){editorSession.value++;slot.value=key;stepIndex.value=index;error.value='';await nextTick();dialog.value.showModal()}
async function apply(a){const draft=JSON.parse(JSON.stringify(c.value));
 if(stepIndex.value!==null)draft.sequences[seqId.value].steps[stepIndex.value]=a;
 else draft.profiles.find(p=>p.id===c.value.active).bindings[slot.value]=a;
 const saved=await act(()=>invoke('save',{value:draft}));
 if(saved){c.value=saved;notice.value='已保存';dialog.value.close()}}
async function addSeq(){const name=seqName.value.trim();if(!name)return;const id=crypto.randomUUID();c.value.sequences[id]={name,steps:[{kind:'wait',ms:300}]};seqId.value=id;await save();seqName.value=''}
async function addProfile(){if(!newProfile.value.trim())return;const p=JSON.parse(JSON.stringify(profile.value));p.id=crypto.randomUUID();p.name=newProfile.value;p.apps=[];c.value.profiles.push(p);c.value.active=p.id;newProfile.value='';await save()}
function download(){const obj=portable(JSON.parse(JSON.stringify(c.value)));const u=URL.createObjectURL(new Blob([JSON.stringify(obj,null,2)],{type:'application/json'}));const a=document.createElement('a');a.href=u;a.download='AU05-profiles.json';a.click();setTimeout(()=>URL.revokeObjectURL(u),5000)}
async function importData(ev){const f=ev.target.files[0];ev.target.value='';if(!f)return;if(f.size>1024*1024)throw Error('配置文件大于 1 MB');const data=JSON.parse(await f.text());const next=mergePortable(JSON.parse(JSON.stringify(c.value)),data);if(confirm('将新增 '+data.profiles.length+' 套配置，是否导入？')){const saved=await invoke('save',{value:next});c.value=saved;notice.value='已导入'}}
const updateMessage=ref('启动时及每六小时自动检查'), newVersion=ref(''), checking=ref(false);
let updateTimer;
async function checkUpdates(){
  if(checking.value)return;
  checking.value=true;
  try {
    const result=await checkRelease(rawRepository.trim(),version);
    newVersion.value=result.version??'';
    updateMessage.value=result.message;
  } catch {
    updateMessage.value='未能检查更新，请稍后重试';
  } finally { checking.value=false }
}
function openProject(releases=false){return act(()=>invoke('open_project',{releases}))}
onMounted(()=>{
  checkUpdates();
  updateTimer=setInterval(checkUpdates,6*60*60*1000);
});
onBeforeUnmount(()=>clearInterval(updateTimer));
onMounted(()=>act(async()=>{systemTheme.addEventListener('change',syncTheme);await load();timer=setInterval(()=>{if(alive)invoke('state').then(value=>s.value=value).catch(e=>error.value=String(e))},850);if(!c.value.setupComplete){await go('连接');await invoke('diagnostic',{value:true});s.value=await invoke('state')}}));
onBeforeUnmount(()=>{alive=false;clearInterval(timer);clearTimeout(noticeTimer);systemTheme.removeEventListener('change',syncTheme)});
</script>
<template><div id="desk" :class="{light:c?.theme==='light'||(c?.theme==='system'&&prefersLight)}">
<div class="title-strip" @mousedown.self.left="windowAction('drag')" @dblclick.self="windowAction('zoom')"><span class="product-title">SSAI-语音控制器</span><div class="window-buttons"><button aria-label="隐藏到托盘" title="隐藏到托盘" @click="windowAction('hide')"></button><button aria-label="最小化" title="最小化" @click="windowAction('minimize')"></button><button aria-label="最大化或还原" title="最大化或还原" @click="windowAction('zoom')"></button></div></div>
<nav aria-label="主导航"><button v-for="[t,icon] in nav" :key="t" :class="{active:t===tab}" :aria-current="t===tab?'page':undefined" @click="act(()=>go(t))"><component :is="icon" :size="25" weight="bold" aria-hidden="true"/>{{t}}</button></nav>
<article><header class="app-header"><div class="heading-group"><h1>{{titles[tab]}}</h1><label v-if="tab==='按键'&&c" class="map-toggle"><input class="switch" type="checkbox" v-model="c.enabled" @change="save">自定义按键</label></div>
<button class="device" :class="{connected}" title="查看设备连接" @click="act(()=>go('连接'))"><span class="device-glyph"><PhMicrophone :size="20" weight="bold" aria-hidden="true"/></span><span class="device-details"><strong>Ulanzi AU05</strong><small><span class="mic-state">{{connected?'已连接':'未连接'}}</span><span class="battery-reading" :title="battery===null?'电量未知':'机身电量 '+battery+'%'"><span class="battery-icon" aria-hidden="true"><i :style="{width:(battery??0)+'%'}"></i></span>{{battery===null?'—':battery+'%'}}</span></small></span><PhCaretDown :size="14"/></button></header>
<section ref="content" class="content" :class="{'mapping-page':tab==='按键'}"><div v-if="error" class="pageError" role="alert"><PhWarningCircle :size="17"/>{{error}}</div>
<div v-if="newVersion" class="release-banner" role="status"><strong>新版本 {{newVersion}} 已发布</strong><button @click="openProject(true)">查看更新 ↗</button></div>
<template v-if="c">
<div v-if="sections[tab]" class="subnav" role="group" :aria-label="titles[tab]+'分区'"><button v-for="item in sections[tab]" :key="item" :aria-pressed="sub===item" @click="selectSection(item)">{{item}}</button></div>
<template v-if="tab==='按键'"><div class="map-tools"><label><PhSlidersHorizontal :size="17"/>当前配置 <select v-model="c.active" aria-label="当前按键配置" @change="save"><option v-for="p in c.profiles" :key="p.id" :value="p.id">{{p.name}}</option></select></label><button class="quiet" @click="act(()=>go('关于','按键配置'))">管理配置 <PhCaretRight :size="14"/></button></div>
<Mapping :connected="connected" :lights="connected?s.direct?.lights:null" :config="c" :profile="profile" @edit="editor" @voice="act(()=>go('连接','语音输入'))"/><ProjectInfo compact @open="openProject()"/></template>
<template v-if="tab==='连接'"><div v-if="!c.setupComplete" class="block">
<h2>首次连接 AU05</h2><ol><li v-for="step in setupSteps" :key="step.label">{{step.label}} · {{step.done?'已完成':'待完成'}}</li></ol>
<p>在「设备连接」启用设备直控，然后到「按键测试」按一次确认键。选好麦克风和语音工具后，在「按键测试」关闭测试模式，再到「语音输入」的测试框按住语音键说话。</p>
<div class="row"><button class="primary" :disabled="!setupSteps.every(step=>step.done)" @click="act(()=>finishSetup(false))">完成设置</button><button @click="act(()=>finishSetup(true))">跳过向导，使用当前设置</button></div></div><DeviceLights v-if="sub==='灯光'" :available="connected"/><DeviceConnection :section="sub" :state="s" :config="c" @control="setVendor" @changed="s=$event" @error="error=$event"/>
<template v-if="sub==='语音输入'"><div class="block"><h2>麦克风</h2><div class="row"><select class="recording-device" aria-label="录音设备" v-model="c.endpoint" @change="save"><option value="">选择录音设备</option><option v-for="d in devices" :key="d">{{d}}</option></select><button @click="act(detect)">重新检测</button><button :disabled="busy||!devices.includes(c.endpoint)" @click="act(listen)">{{busy?'正在录制…':'试听 3 秒'}}</button><button @click="act(()=>invoke('voice_settings'))">系统声音设置</button></div><audio v-if="audio" :src="audio" controls></audio><p v-if="c.endpoint&&!devices.includes(c.endpoint)" class="err">已选麦克风离线，请重新选择。</p><p class="muted">使用外部输入工具时，也请在该工具中选中 AU05 麦克风。</p></div>
<div class="block"><h2>语音输入工具</h2><div class="row"><select class="voice-tool" aria-label="语音输入工具" v-model="c.tool" @change="save"><option v-for="t in ['微信输入法','豆包','Vokie','Typeless','Windows 系统识别','自定义']">{{t}}</option></select><button v-if="c.tool!=='Windows 系统识别'" aria-label="编辑语音快捷键" @click="editVoiceShortcut">{{c.voiceKeys.join(' + ')}}</button><label v-if="c.tool!=='Windows 系统识别'"><input type="checkbox" v-model="c.toggleVoice" @change="save">按下与松开各点按一次</label></div><p v-if="c.tool!=='Windows 系统识别'" class="muted">按住 AU05 语音键说话，松开后在当前输入框查看结果。更换输入工具时请核对触发快捷键。</p><p><label><input type="checkbox" v-model="c.voiceEnabled" @change="save">启用 K1 语音触发</label></p><textarea v-model="testText" @input="confirmedText=false" placeholder="点击这里，用 AU05 测试听写"></textarea><small>输入框内容仅用于本次测试。</small>
<p v-if="s.diagnostic">测试模式只显示事件。请先到「按键测试」关闭测试模式，再回到此处听写。</p>
<label v-if="!c.setupComplete"><input v-model="confirmedText" type="checkbox" :disabled="s.diagnostic||!testText.trim()">我已在输入框看到本次听写结果</label></div></template></template>
<template v-if="tab==='组合动作'"><div class="block"><h2>组合动作</h2><div class="row"><select aria-label="选择组合" v-model="seqId"><option value="">选择组合</option><option v-for="(v,k) in c.sequences" :value="k">{{v.name}}</option></select><input v-model="seqName" placeholder="新组合名称"><button class="primary" :disabled="!seqName.trim()" @click="addSeq">新建</button></div><div v-if="!seq" class="empty"><PhStack :size="26"/><strong>{{Object.keys(c.sequences).length?'选择要编辑的组合':'还没有组合动作'}}</strong><small>输入名称并新建组合，再按执行顺序添加步骤。</small></div><template v-if="seq"><p class="row"><label>名称 <input v-model="seq.name" @change="save"></label>已绑定 {{bindingCount(seqId)}} 处 <button @click="duplicateSeq">复制</button><button class="danger" @click="removeSeq">删除组合</button></p><ol class="sequence-steps"><li v-for="(a,i) in seq.steps" :key="i"><button class="step-action" @click="editor('',i)">{{describe(a,c)}}</button> <button :disabled="i===0" @click="seq.steps.splice(i-1,0,seq.steps.splice(i,1)[0]);save()">上移</button> <button :disabled="seq.steps.length===1" @click="seq.steps.splice(i,1);save()">移除</button></li></ol><button @click="editor('',seq.steps.length)">添加步骤</button> <button @click="act(()=>invoke('try_sequence',{id:seqId}))">试运行</button> <button @click="act(()=>invoke('cancel'))">停止运行</button></template></div></template>
<StatsPanel v-if="tab==='统计'" :counts="counts" :rank="rank" :span="span" @period="span=$event;act(totals)" @clear="act(async()=>{if(confirm('清空本机按键与时长统计？')){await invoke('clear_stats');await totals()}})"/>
<template v-if="tab==='回眸'"><HistoryPanel :section="sub" @section="selectSection" :config="c" @saved="c=$event;notice='已保存'" @error="error=$event" @notice="notice=$event"/>
<div v-if="sub==='客户端授权'" class="block"><h2>本地 Agent 访问</h2><label><input type="checkbox" v-model="c.agent" @change="save">允许已授权客户端只读访问</label><p class="muted">授权范围为本机已保存的回眸，可按日期和应用检索。客户端可以把读到的文本发送给其模型。</p><div class="row"><input v-model="agentName" placeholder="客户端名称"><button :disabled="!c.agent||!agentName.trim()" @click="act(grant)">创建授权</button></div><textarea v-if="snippet" readonly :value="snippet" rows="5"></textarea><p v-for="g in grants">{{g.name}} <button @click="act(async()=>{await invoke('revoke',{id:g.id});snippet='';await fetchGrants()})">撤销</button></p><button v-if="grants.length" @click="act(async()=>{if(confirm('撤销全部客户端授权？')){await invoke('revoke_all');snippet='';await fetchGrants()}})">撤销全部授权</button></div></template>
<template v-if="tab==='权限'"><div class="block"><h2>系统权限</h2>
 <div class="setting-row"><div><strong>麦克风访问</strong><p class="muted">试听使用选定的麦克风。微信输入法等外部工具需单独允许麦克风访问。</p></div><button @click="act(()=>invoke('voice_settings'))">系统声音设置</button></div>
 <div class="setting-row"><div><strong>模拟按键</strong><p class="muted">若高权限窗口无法接收快捷键，请在普通编辑器中复测。</p></div><button @click="act(()=>go('连接','按键测试'))">测试按键</button></div></div>
 <div class="block"><div class="setting-row"><h2>诊断摘要</h2><button @click="act(()=>copy(diagnosis()))">复制摘要</button></div><pre>{{diagnosis()}}</pre></div>
 <div class="block"><div class="setting-row"><div><h2>连接向导</h2><p class="muted">依次检查设备、麦克风和听写结果。</p></div><button @click="act(startSetup)">重新运行向导</button></div></div></template>
<template v-if="tab==='关于'"><div v-if="sub==='应用设置'" class="block"><div class="about-app"><span class="device-glyph"><PhMicrophone :size="25"/></span><div><h2>SSAI-语音控制器</h2><small>独立桌面端 · {{version}}</small></div></div>
 <ProjectInfo @open="openProject()"/>
 <div class="setting-row"><div><strong>软件更新</strong><p class="muted" role="status">{{updateMessage}}</p></div><button :disabled="checking" @click="checkUpdates">{{checking?'正在检查…':'检查更新'}}</button></div>
 <div class="setting-row"><div><strong>外观</strong><p class="muted">选择桌面软件的配色。</p></div><select aria-label="外观" v-model="c.theme" @change="save"><option value="dark">深色</option><option value="light">浅色</option><option value="system">跟随系统</option></select></div>
 <div class="setting-row"><strong>登录时启动</strong><input class="switch" aria-label="登录时启动" type="checkbox" :checked="loginEnabled" @change="act(()=>login($event.target.checked)).finally(()=>{$event.target.checked=loginEnabled})"></div></div><div v-if="sub==='按键配置'" class="block"><h2>按键配置</h2><div class="row"><select aria-label="管理按键配置" v-model="c.active" @change="save"><option v-for="p in c.profiles" :value="p.id">{{p.name}}</option></select><input v-model="newProfile" placeholder="新配置名"><button @click="addProfile">复制为新配置</button></div><p class="row"><label>当前名称 <input v-model="profile.name" @change="save"></label><button :disabled="profile.id==='default'" @click="removeProfile">删除当前配置</button></p><p><label><input type="checkbox" v-model="c.follow" @change="save">按前台应用匹配配置</label></p><input :value="profile.apps.join(',')" placeholder="关联应用，例如 notepad.exe" @change="profile.apps=$event.target.value.split(',').map(s=>s.trim()).filter(Boolean);save()"></div></template>
<template v-if="tab==='分享'">
  <div class="block"><h2>导出按键方案</h2><p class="muted">文件包含按键绑定及组合步骤，可在另一台电脑上导入。</p>
  <ul class="profile-list"><li v-for="p in c.profiles" :key="p.id"><PhSlidersHorizontal :size="17"/><strong>{{p.name}}</strong><small>{{Object.values(p.bindings).filter(a=>a.kind!=='none').length}} 项绑定</small></li></ul>
  <p class="muted">导出内容排除短语正文、应用路径、历史和授权凭据。</p><button class="primary" @click="act(download)">导出 JSON</button></div>
  <div class="block"><div class="setting-row"><div><h2>导入方案</h2><p class="muted">选择 AU05 的 JSON 文件，确认后另存为新配置。</p></div><button @click="importFile.click()">选择文件</button></div><input type="file" hidden ref="importFile" accept=".json" @change="act(()=>importData($event))"></div></template>
<dialog ref="voiceDialog" aria-labelledby="voice-shortcut-title" @close="editingVoice=false"><div v-if="editingVoice" class="editor stack"><h2 id="voice-shortcut-title">语音快捷键</h2><ShortcutPicker v-model="voiceDraft"/><p v-if="error" class="pageError" role="alert">{{error}}</p><div class="dialog-actions"><button @click="voiceDialog.close()">取消</button><button class="primary" :disabled="!voiceDraft.trim()" @click="saveVoiceShortcut">保存快捷键</button></div></div></dialog>
<dialog ref="dialog" aria-labelledby="action-title"><ActionEditor :heading="editorTitle" :slot-key="slot" :status="s.message" :error="error" v-if="profile" :key="editorSession" :config="c" :action="stepIndex===null?profile.bindings[slot]:seq?.steps[stepIndex]" :step="stepIndex!==null" @apply="apply" @close="dialog.close()"/></dialog>
</template></section><footer role="status"><PhKeyboard :size="15"/><button v-if="s.diagnostic && tab!=='连接'" @click="act(()=>go('连接','按键测试'))">测试模式 · 查看事件</button>{{s.message??'正在连接桌面服务'}} <span v-if="notice" class="notice"><PhCheckCircle :size="15"/>{{notice}}</span></footer></article></div></template>
