<script setup>
defineProps({
  selected: { type: Object, required: true },
  statusLabel: { type: Function, required: true },
  markClass: { type: Function, required: true },
});

const emit = defineEmits(["copy", "open-local"]);
</script>

<template>
  <article class="detail" :class="selected.status">
    <div class="detail-head">
      <span class="pill" :class="markClass(selected)">{{ statusLabel(selected.status) }}{{ selected.required ? "" : " · 推荐" }}</span>
    </div>
    <dl class="facts">
      <div v-for="(f, fi) in (selected.facts || [])" :key="fi" class="fact">
        <dt>{{ f.label }}</dt>
        <dd>
          <span :title="f.value">{{ f.value }}</span>
          <button
            v-if="f.value && f.value !== '未知' && f.value !== '未设置' && f.value !== '—'"
            class="copy-mini"
            @click="emit('copy', f.value)"
          >复制</button>
        </dd>
      </div>
    </dl>
    <div class="installs" v-if="selected.installs && selected.installs.length">
      <p class="installs-label">本机发现的安装</p>
      <ul>
        <li v-for="(ins, ii) in selected.installs" :key="ii">
          <span class="src">{{ ins.source }}</span>
          <span class="ver">{{ ins.version || "版本未知" }}</span>
          <span class="now" v-if="ins.active">当前使用</span>
          <button class="path-line" :title="ins.homeDir" @click="emit('copy', ins.homeDir)">{{ ins.homeDir }}</button>
        </li>
      </ul>
    </div>
    <div class="configs" v-if="selected.configs && selected.configs.length">
      <article v-for="(cfg, ci) in selected.configs" :key="ci" class="config-block" :class="{ missing: !cfg.exists }">
        <div class="config-head">
          <strong>{{ cfg.kind }}</strong>
          <span class="pill sm" :class="cfg.exists ? 'ok' : 'warn'">{{ cfg.exists ? "已找到" : "未找到" }}</span>
        </div>
        <p class="mono" :title="cfg.path">{{ cfg.path }}</p>
        <dl class="facts tight" v-if="cfg.facts && cfg.facts.length">
          <div v-for="(f, fi) in cfg.facts" :key="fi" class="fact">
            <dt>{{ f.label }}</dt>
            <dd>
              <span :title="f.value">{{ f.value }}</span>
              <button
                v-if="f.value && f.value !== '未知' && f.value !== '未配置' && f.value !== '未找到配置文件' && f.value !== '文件不存在'"
                class="copy-mini"
                @click="emit('copy', f.value)"
              >复制</button>
            </dd>
          </div>
        </dl>
        <pre class="preview" v-if="cfg.preview">{{ cfg.preview }}</pre>
        <div class="card-actions">
          <button class="btn ghost" :disabled="!cfg.path" @click="emit('open-local', { path: cfg.path, mode: 'folder' })">打开文件夹</button>
          <button class="btn ghost" :disabled="!cfg.exists" @click="emit('open-local', { path: cfg.path, mode: 'file' })">打开文件</button>
        </div>
      </article>
    </div>
  </article>
</template>
