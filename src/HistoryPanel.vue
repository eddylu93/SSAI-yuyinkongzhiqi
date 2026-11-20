<script setup>
import {ref, watch, nextTick} from 'vue';
import {invoke} from '@tauri-apps/api/core';

const props = defineProps({config: Object, section: String});
const emit = defineEmits(['saved', 'error', 'notice','section']);
const search = ref(''), date = ref(''), app = ref(''), page = ref(0);
const rows = ref([]), apps = ref([]), more = ref(false);
const name = ref(''), body = ref(''), editor = ref(null);
let generation = 0;
async function load() {
  const version = ++generation;
  const start = date.value ? new Date(date.value + 'T00:00:00') : null;
  const end = start && new Date(start);
  if (end) end.setDate(end.getDate() + 1);
  try {
    const result = await invoke('browse_history', {search: search.value, app: app.value,
      since: start ? start.getTime()/1000 : 0, until: end ? end.getTime()/1000 : null, offset: page.value * 50});
    if (version !== generation) return;
    apps.value = result.apps;
    more.value = result.rows.length > 50;
    rows.value = result.rows.slice(0, 50);
    if (!rows.value.length && page.value > 0) page.value--;
  } catch (e) { if (version === generation) emit('error', String(e)); }
}
watch([search, date, app], () => { if (page.value) page.value = 0; else load(); });
watch(page, load, {immediate: true});
async function update(change) {
  try {
    const next = JSON.parse(JSON.stringify(props.config));
    change(next);
    const saved = await invoke('save', {value: next});
    emit('saved', saved);
    return true;
  } catch (e) { emit('error', String(e)); return false; }
}
async function erase(id) {
  if (!confirm(id ? '删除这条回眸？' : '永久清空本机全部回眸？')) return;
  try {
    await invoke(id ? 'delete_entry' : 'clear_history', id ? {id} : {});
    await load();
  } catch (e) { emit('error', String(e)); }
}
function draft(row) {
  name.value = '';
  body.value = row.text;
  emit('section','常用语');
  nextTick(()=>editor.value?.focus());
}
async function savePhrase() {
  const key = name.value.trim();
  if (!key || !body.value) return;
  if (['__proto__','constructor','prototype'].includes(key)) {emit('error','请选择其他短语名称');return;}
  if (Object.hasOwn(props.config.phrases,key) && !confirm('覆盖同名短语？')) return;
  await update(next => { next.phrases[key] = body.value; });
}
async function deletePhrase() {
  const key = name.value.trim();
  const uses = a => a.kind === 'text' && a.phrase === key;
  const actions = props.config.profiles.flatMap(p => Object.values(p.bindings));
  const sequences = Object.values(props.config.sequences);
  const affected = actions.filter(uses).length + sequences.flatMap(s => s.steps).filter(uses).length;
  if (!confirm('删除「'+key+'」并清除 '+affected+' 处引用？')) return;
  if (await update(next => {
    delete next.phrases[key];
    for (const p of next.profiles) for (const [slot,a] of Object.entries(p.bindings)) {
      if (uses(a)) delete p.bindings[slot];
    }
    for (const s of Object.values(next.sequences)) {
      s.steps = s.steps.filter(a => !uses(a));
      if (!s.steps.length) s.steps.push({kind:'none'});
    }
  })) { name.value=''; body.value=''; }
}
async function copy(text) {
  try { await navigator.clipboard.writeText(text); emit('notice','已复制'); }
  catch(e){emit('error',String(e));}
}
</script>
<template>
  <div v-show="section==='听写记录'" class="block"><h2>记录与筛选</h2>
    <label><input type="checkbox" :checked="config.history" @change="update(c=>{c.history=$event.target.checked})">记录已确认的最终文本</label>
    <p class="muted">启用后，Windows 系统识别会保存已确认提交的文本和应用名称。外部热键工具的听写结果无法回传。</p>
    <div class="row"><input v-model="search" placeholder="搜索历史"><input v-model="date" type="date" aria-label="回眸日期">
      <select v-model="app" aria-label="筛选应用"><option value="">全部应用</option><option v-for="a in apps" :key="a">{{a}}</option></select></div>
    <ul class="list"><li v-if="!rows.length" class="empty">没有符合条件的回眸</li><li v-for="r in rows" :key="r.id">
      <small>{{new Date(r.stamp*1000).toLocaleString()}} · {{r.app}}</small><p>{{r.text}}</p>
      <button @click="copy(r.text)">复制</button> <button @click="draft(r)">另存为常用语</button> <button @click="erase(r.id)">删除</button>
    </li></ul><div class="row"><button :disabled="!page" @click="page--">上一页</button><button :disabled="!more" @click="page++">下一页</button><button :disabled="!apps.length" @click="erase(null)">清空全部回眸</button></div>
  </div>
  <div v-show="section==='常用语'" class="block"><h2>常用语</h2><div class="row"><input ref="editor" v-model="name" placeholder="名称">
    <select :value="name" aria-label="选择已有短语" @change="name=$event.target.value;body=config.phrases[name]??''"><option value="">新短语</option><option v-for="(_,k) in config.phrases" :key="k">{{k}}</option></select></div>
    <p><textarea v-model="body" placeholder="输入短语正文"></textarea></p>
    <div class="row"><button class="primary" :disabled="!name.trim()||!body" @click="savePhrase">保存常用语</button>
      <button :disabled="!Object.hasOwn(config.phrases,name.trim())" @click="deletePhrase">删除常用语</button><button :disabled="!body" @click="copy(body)">复制正文</button></div>
  </div>
</template>
