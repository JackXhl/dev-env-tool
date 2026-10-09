<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import logoUrl from "./assets/logo.png";
import AppSidebar from "./components/AppSidebar.vue";
import AppOverview from "./components/AppOverview.vue";
import AppToolDetail from "./components/AppToolDetail.vue";

const brand = ref({
  penName: "项目环境探查",
  repo: "",
  github: "https://github.com/JackXhl",
  qq: "",
  about: "",
});
const links = ref({});
const scanning = ref(false);
const items = ref([]);
const revealed = ref(0);
const aboutOn = ref(false);
const selectedId = ref("overview");
const toast = ref("");
let toastTimer = 0;
let unsubs = [];

const SPRING_IDS = ["jdk", "maven", "mysql", "redis", "nginx"];
const NODE_IDS = ["nvm", "node", "pnpm"];

const springItems = computed(() => items.value.filter((i) => SPRING_IDS.includes(i.id)));
const nodeItems = computed(() => items.value.filter((i) => NODE_IDS.includes(i.id)));
const selected = computed(() => items.value.find((i) => i.id === selectedId.value) || null);
const coreIssues = computed(() => springItems.value.filter((i) => i.status !== "ok"));
const okCount = computed(() => items.value.filter((i) => i.status === "ok").length);
const scanPct = computed(() => {
  if (!items.value.length) return scanning.value ? 12 : 0;
  return Math.round((revealed.value / Math.max(items.value.length, 1)) * 100);
});
const hero = computed(() => {
  if (scanning.value) {
    return { title: "正在探查本机环境", sub: "读取 PATH、注册表和常见安装目录" };
  }
  if (!items.value.length) {
    return { title: "尚未探查", sub: "点右上角重新探查，列出本机开发环境" };
  }
  if (coreIssues.value.length === 0) {
    return {
      title: "Spring Boot 环境已就绪",
      sub: nodeItems.value.some((i) => i.status !== "ok")
        ? "核心环境正常。前端推荐用 nvm 管理 Node.js 和 pnpm。"
        : `${okCount.value} 项均已找到。缺项可按教程和下载页安装。`,
    };
  }
  return {
    title: `${coreIssues.value.length} 项需要处理`,
    sub: "左侧选一项查看路径。未安装的点「安装教程」或「下载」。",
  };
});

function itemOf(id) {
  return items.value.find((i) => i.id === id);
}
function statusLabel(s) {
  return {
    ok: "正常",
    not_installed: "未安装",
    wrong_version: "版本不符",
    not_running: "未在运行",
    port_busy: "端口被占用",
  }[s] || s;
}
function linkOf(item) {
  return links.value[item.linkKey] || {};
}
function markClass(item) {
  return item.status === "ok" ? "ok" : "warn";
}
function dotClass(id) {
  const it = itemOf(id);
  if (!it) return scanning.value ? "scan" : "idle";
  return it.status === "ok" ? "ok" : "warn";
}

function showToast(msg) {
  toast.value = String(msg);
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 4200);
}

async function openLocal(path, mode) {
  if (!path) return;
  try {
    await invoke("open_local_path", { path, mode });
  } catch (e) {
    showToast(e);
  }
}

async function scan() {
  scanning.value = true;
  items.value = [];
  revealed.value = 0;
  try {
    const list = await invoke("scan_env");
    items.value = list;
    revealed.value = list.length;
  } catch (e) {
    showToast(e);
  } finally {
    scanning.value = false;
  }
}

async function copy(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    showToast("复制失败");
  }
}

async function openLink(url) {
  if (!url) return;
  try {
    await invoke("open_url", { url });
  } catch (e) {
    showToast(e);
  }
}

onMounted(async () => {
  try {
    brand.value = await invoke("load_brand");
    links.value = await invoke("load_links");
  } catch (e) {
    showToast(e);
  }
  unsubs.push(
    await listen("scan-progress", (e) => {
      const it = e.payload;
      if (!it?.id) return;
      const idx = items.value.findIndex((x) => x.id === it.id);
      if (idx >= 0) items.value[idx] = it;
      else items.value.push(it);
      revealed.value = items.value.length;
    })
  );
  await scan();
});

onUnmounted(() => {
  unsubs.forEach((u) => u());
  window.clearTimeout(toastTimer);
});
</script>

<template>
  <div class="shell">
    <AppSidebar
      :brand="brand"
      :logo-url="logoUrl"
      :selected-id="selectedId"
      :spring-ids="SPRING_IDS"
      :node-ids="NODE_IDS"
      :item-of="itemOf"
      :dot-class="dotClass"
      @select="selectedId = $event"
      @open-repo="openLink(brand.repo)"
      @about="aboutOn = true"
    />

    <section class="workspace">
      <header class="bar">
        <div class="bar-title">
          <div class="bar-row">
            <h1>{{ selected ? selected.name : hero.title }}</h1>
            <div class="bar-guide" v-if="selected && linkOf(selected).tutorial">
              <button class="btn teal" @click="openLink(linkOf(selected).tutorial)">安装教程</button>
              <button
                class="btn ghost"
                v-if="linkOf(selected).download && (selected.status === 'not_installed' || selected.status === 'wrong_version')"
                @click="openLink(linkOf(selected).download)"
              >下载</button>
            </div>
          </div>
          <p>{{ selected ? selected.summary : hero.sub }}</p>
        </div>
        <button class="btn primary" :disabled="scanning" @click="scan">
          {{ scanning ? "探查中…" : "重新探查" }}
        </button>
      </header>
      <div class="meter" :class="{ on: scanning }"><span :style="{ width: scanPct + '%' }"></span></div>

      <main class="board">
        <AppOverview
          v-if="selectedId === 'overview'"
          :items="items"
          :ok-count="okCount"
          :status-label="statusLabel"
          :mark-class="markClass"
          @select="selectedId = $event"
        />
        <AppToolDetail
          v-else-if="selected"
          :selected="selected"
          :status-label="statusLabel"
          :mark-class="markClass"
          @copy="copy"
          @open-local="openLocal($event.path, $event.mode)"
        />
      </main>
    </section>

    <div class="toast" v-if="toast">{{ toast }}</div>

    <div class="modal" v-if="aboutOn" @click.self="aboutOn = false">
      <div class="panel">
        <h2>关于 {{ brand.penName }}</h2>
        <p>{{ brand.about }}</p>
        <p v-if="brand.repo">
          仓库（GitHub）：
          <button type="button" class="copy-mini" @click="openLink(brand.repo)">{{ brand.repo }}</button>
        </p>
        <p v-if="brand.github">
          作者主页：
          <button type="button" class="copy-mini" @click="openLink(brand.github)">{{ brand.github }}</button>
        </p>
        <p v-if="brand.qq">QQ：{{ brand.qq }} <button class="copy-mini" @click="copy(brand.qq)">复制</button></p>
        <div class="row-btns">
          <button class="btn ghost" @click="aboutOn = false">关闭</button>
        </div>
      </div>
    </div>
  </div>
</template>
