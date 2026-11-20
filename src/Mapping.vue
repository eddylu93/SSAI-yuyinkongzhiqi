<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { PhArrowArcLeft, PhArrowArcRight, PhCircle, PhCheckCircle, PhMicrophone, PhXCircle } from '@phosphor-icons/vue';
import { groups, gestures, describe } from './mappings.js';
import DeviceModel from './DeviceModel.vue';

const props = defineProps({config: Object, profile: Object, lights:Object, connected:Boolean});
const emit = defineEmits(['edit', 'voice']);
const picked = ref(6);
const board = ref(null);
const device = ref(null);
const cards = [];
const lines = ref([]);
let ports=[];
function projection(points){ports=points;updateLines()}
const icons = [PhArrowArcLeft, PhCircle, PhArrowArcRight, PhArrowArcLeft, PhArrowArcRight, PhMicrophone, PhCheckCircle, PhXCircle];
let observer;

function updateLines() {
  if (!board.value || !device.value) return;
  const bounds = board.value.getBoundingClientRect();
  const unitBox = device.value.getBoundingClientRect();
  lines.value = ports.map(([fx, fy], i) => {
    if (!cards[i]) return '';
    const card = cards[i].getBoundingClientRect();
    const x = unitBox.left - bounds.left + unitBox.width * fx;
    const y = unitBox.top - bounds.top + unitBox.height * fy;
    const end = (i < 5 ? card.right + 6 : card.left - 6) - bounds.left;
    const endY = card.top + card.height / 2 - bounds.top;
    const mid = (x + end) / 2;
    return {x,y,path:`M ${x} ${y} C ${mid} ${y} ${mid} ${endY} ${end} ${endY}`};
  });
}
onMounted(() => {
  observer = new ResizeObserver(updateLines);
  [board.value, device.value, ...cards].forEach(el => observer.observe(el));
  nextTick(updateLines);
});
onBeforeUnmount(() => observer?.disconnect());
function edit(i, gesture) {
  picked.value = i;
  if (gesture) emit('edit', groups[i].control + '.' + gesture);
  else emit('voice');
}
</script>
<template>
  <div class="map" ref="board">
    <div v-for="(indices, side) in [[0,1,2,3,4],[5,6,7]]" :key="side" class="groups" :class="side?'right':'left'">
      <div v-for="i in indices" :key="i" :ref="el => cards[i]=el" class="card" :class="{pick:picked===i}" @click="picked=i">
        <strong><component :is="icons[i]" :size="15"/>{{groups[i].name}}<small v-if="i===5" class="voice-tag">语音触发键</small></strong>
        <div v-if="groups[i].gestures.length" class="slots">
          <button v-for="k in groups[i].gestures" :key="k" :class="{unbound:!profile.bindings[groups[i].control+'.'+k]||profile.bindings[groups[i].control+'.'+k].kind==='none'}" :aria-label="groups[i].name+' '+gestures[k]+' · '+describe(profile.bindings[groups[i].control+'.'+k],config)" @click="edit(i,k)">
            <small>{{gestures[k]}}</small>{{describe(profile.bindings[groups[i].control+'.'+k],config)}}
          </button>
        </div>
        <button v-else class="voice-link" @click="edit(i)"><span>{{config.tool}}</span><small><i class="dot" :class="{online:config.voiceEnabled}"/>{{config.voiceEnabled?'按住说话已启用':'设置按住说话'}}</small></button>
      </div>
    </div>
    <div class="product" ref="device">
      <DeviceModel :lights="lights" :connected="connected" :picked="picked" @projection="projection" @pick="picked=$event"/>
    </div>
    <svg class="wires" aria-hidden="true">
      <defs><marker v-for="(color,id) in {normal:'var(--wire)',selected:'var(--accent)'}" :key="id" :id="id" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="6" markerHeight="6" orient="auto"><path d="M0 0 L8 4 L0 8 Z" :fill="color"/></marker></defs>
      <g v-for="(line,i) in lines" :key="i"><circle :cx="line.x" :cy="line.y" :r="picked===i?2.4:1.6" :fill="picked===i?'var(--accent)':'var(--wire)'"/><path :d="line.path" fill="none" :stroke="picked===i?'var(--accent)':'var(--wire)'" :stroke-width="picked===i?1.5:1" :marker-end="'url(#'+(picked===i?'selected':'normal')+')'"/></g>
    </svg>
  </div>
</template>
