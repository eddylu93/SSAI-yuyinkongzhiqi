<script setup>
import { computed } from 'vue';
import { PhBatteryCharging } from '@phosphor-icons/vue';

const props = defineProps({ state: Object, detailed: Boolean });
const online = computed(() => props.state?.online && props.state.direct?.transmitter === true);
const level = computed(() => {
  const value = props.state?.direct?.battery;
  return online.value && Number.isInteger(value) && value >= 0 && value <= 100 ? value : null;
});
const charging = computed(() => online.value && props.state.direct.charging === true);
const description = computed(() => [
  charging.value ? '充电中' : '', level.value === null ? '电量未知' : '机身电量 ' + level.value + '%'
].filter(Boolean).join('，'));
</script>

<template>
  <span class="battery-reading" :class="{charging}" role="img" :aria-label="description" :title="description">
    <PhBatteryCharging v-if="charging" :size="20" weight="bold" aria-hidden="true"/>
    <span v-else class="battery-icon" aria-hidden="true"><i :style="{width:(level??0)+'%'}"></i></span>
    {{ level === null ? (detailed ? '未知' : '—') : level + '%' }}
    <span v-if="charging && detailed">充电中</span>
  </span>
</template>
