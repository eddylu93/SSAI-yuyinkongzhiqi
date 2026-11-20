<script setup>
import {computed} from 'vue';
import {PhClock, PhKeyboard, PhMicrophone, PhChartBar} from '@phosphor-icons/vue';
const props=defineProps({counts:Array,rank:Array,span:String});
defineEmits(['period','clear']);
const labels={K1:'语音键',K2:'确认键',K3:'取消键',DIAL:'旋钮按下','DIAL.turn':'旋钮转动'};
const voice=computed(()=>props.counts.find(r=>r.control==='K1'));
const max=computed(()=>Math.max(1,...props.counts.map(r=>r.count)));
const presses=computed(()=>props.counts.reduce((n,r)=>n+(r.control==='DIAL.turn'?0:r.count),0));
function duration(ms=0){
  if(ms<60000)return (ms/1000).toFixed(ms?1:0)+' 秒';
  return Math.floor(ms/60000)+' 分 '+Math.floor(ms/1000%60)+' 秒';
}
</script>
<template>
  <div class="toolbar"><h2>活动概览</h2><div class="segments" role="group" aria-label="统计周期"><button v-for="v in ['今日','本周','全部']" :key="v" :aria-pressed="span===v" @click="$emit('period',v)">{{v}}</button></div></div>
  <div class="metrics">
    <div class="metric"><span><PhKeyboard/>实体按压</span><strong>{{presses}} <small>次</small></strong></div>
    <div class="metric"><span><PhMicrophone/>语音键使用时长</span><strong>{{duration(voice?.ms)}}</strong></div>
    <div class="metric"><span><PhClock/>最长单次语音</span><strong>{{duration(voice?.longest)}}</strong></div>
  </div>
  <div class="block"><h2>按键使用分布</h2>
    <div v-if="!counts.length" class="empty"><PhChartBar :size="28"/><strong>这个时段还没有使用记录</strong><small>关闭按键测试后，实体操作会计入统计。</small></div>
    <div v-else class="usage-list"><div v-for="row in counts" :key="row.control"><span>{{labels[row.control]??row.control}}</span><div class="track"><i :style="{width:(100*row.count/max)+'%'}"/></div><b>{{row.count}} {{row.control==='DIAL.turn'?'步':'次'}}</b></div></div>
    <p v-if="voice?.interrupted" class="muted">中断的语音会话 {{voice.interrupted}} 次</p>
  </div>
  <div v-if="rank.length" class="block"><h2>最长单次语音</h2><ol class="ranking"><li v-for="(r,i) in rank" :key="i"><time>{{new Date(r.ended_at_unix_ms).toLocaleString()}}</time><b>{{duration(r.duration_seconds*1000)}}</b></li></ol></div>
  <button class="danger" @click="$emit('clear')">清空统计</button>
</template>
