<script setup>
defineProps({
  items: { type: Array, required: true },
  okCount: { type: Number, required: true },
  statusLabel: { type: Function, required: true },
  markClass: { type: Function, required: true },
});

const emit = defineEmits(["select"]);
</script>

<template>
  <div class="dash">
    <article class="stat">
      <span>已探查</span>
      <strong>{{ items.length }}</strong>
    </article>
    <article class="stat">
      <span>正常</span>
      <strong class="ok">{{ okCount }}</strong>
    </article>
    <article class="stat">
      <span>待处理</span>
      <strong class="warn">{{ items.length - okCount }}</strong>
    </article>
    <button
      v-for="item in items"
      :key="item.id"
      type="button"
      class="dash-card"
      :class="item.status"
      @click="emit('select', item.id)"
    >
      <div class="dash-top">
        <strong>{{ item.name }}</strong>
        <span class="pill" :class="markClass(item)">{{ statusLabel(item.status) }}</span>
      </div>
      <p>{{ item.summary }}</p>
      <code v-if="item.version && item.version !== '未知'">{{ item.version }}</code>
    </button>
  </div>
</template>
