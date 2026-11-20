<script setup>
import rawRepository from './repository.txt?raw'
const repository = rawRepository.trim()
defineProps({compact: Boolean})
defineEmits(['open'])
</script>

<template>
  <div class="project-info">
    <span>作者 三笙</span><span>微信 sanshengya88</span>
    <a :href="repository" :title="repository" @click.prevent="$emit('open')">{{compact ? 'GitHub 仓库 ↗' : repository}}</a>
  </div>
</template>
