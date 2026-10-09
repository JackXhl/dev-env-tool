<script setup>
defineProps({
  brand: { type: Object, required: true },
  logoUrl: { type: String, required: true },
  selectedId: { type: String, required: true },
  springIds: { type: Array, required: true },
  nodeIds: { type: Array, required: true },
  itemOf: { type: Function, required: true },
  dotClass: { type: Function, required: true },
});

const emit = defineEmits(["select", "open-repo", "about"]);
</script>

<template>
  <aside class="rail">
    <button class="brand" type="button" @click="emit('select', 'overview')">
      <img :src="logoUrl" alt="" />
      <span>
        <strong>{{ brand.penName }}</strong>
        <small>本机扫描</small>
      </span>
    </button>

    <nav>
      <button type="button" :class="{ on: selectedId === 'overview' }" @click="emit('select', 'overview')">
        <i class="dot idle"></i>
        概览
      </button>
      <p class="nav-label">Spring Boot</p>
      <button
        v-for="id in springIds"
        :key="id"
        type="button"
        :class="{ on: selectedId === id }"
        @click="emit('select', id)"
      >
        <i class="dot" :class="dotClass(id)"></i>
        {{ itemOf(id)?.name || id }}
      </button>
      <p class="nav-label">前端</p>
      <button
        v-for="id in nodeIds"
        :key="id"
        type="button"
        :class="{ on: selectedId === id }"
        @click="emit('select', id)"
      >
        <i class="dot" :class="dotClass(id)"></i>
        {{ itemOf(id)?.name || id }}
      </button>
    </nav>

    <div class="rail-foot">
      <button type="button" v-if="brand.repo" @click="emit('open-repo')">GitHub</button>
      <button type="button" @click="emit('about')">关于</button>
    </div>
  </aside>
</template>
