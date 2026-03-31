<script setup lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { useToast } from 'primevue/usetoast';
  import { ref, onMounted } from 'vue';

  import { commands, type BuildOptions, type RunOptions } from './bindings';

  const toast = useToast();

  const activeTab = ref('0');

  const inputs = ref('');
  const fonts = ref('');

  // Process options
  const processOptions = ref<RunOptions>({
    inputs: [],
    output: '',
    fontpaths: null,
    dbpath: '',
    strict: true,
    allow_missing_sample: false,
    allow_missing_fonts: false,
    report: false,
    force: false,
  });

  // Build options
  const buildOptions = ref<BuildOptions>({
    fontpaths: [],
    output: '',
  });

  const isProcessing = ref(false);
  const progress = ref(0);
  const logs = ref<string[]>([]);

  const addLog = (msg: string) => {
    logs.value.push(`[${new Date().toLocaleTimeString()}] ${msg}`);
    if (logs.value.length > 100) logs.value.shift();
  };

  onMounted(async () => {
    await listen('process-updated', (event: { payload: number }) => {
      progress.value = event.payload;
      addLog(`Progress: ${event.payload}%`);
    });
  });

  async function pickFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: 'Subtitle', extensions: ['ass'] }],
    });
    if (selected) {
      let inp = inputs.value.split('|');
      inp.push(...selected);
      inputs.value = inp.join('|');
    }
  }

  async function pickDirs() {
    const selected = await open({
      multiple: true,
      directory: true,
      filters: [{ name: 'Subtitle', extensions: ['ass'] }],
    });
    if (selected) {
      let inp = inputs.value.split('|');
      inp.push(...selected);
      inputs.value = inp.join('|');
    }
  }

  async function pickOutputDir() {
    const selected = await open({
      directory: true,
    });
    if (selected && typeof selected === 'string') {
      processOptions.value.output = selected;
    }
  }

  async function pickFontDirs() {
    const selected = await open({
      directory: true,
      multiple: true,
    });
    if (selected) {
      let f = fonts.value.split('|');
      f.push(...selected);
      fonts.value = f.join('|');
    }
  }

  async function pickDbDir() {
    const selected = await open({
      directory: true,
    });
    if (selected && typeof selected === 'string') {
      processOptions.value.dbpath = selected;
    }
  }

  async function pickBuildFontDirs() {
    const selected = await open({
      directory: true,
      multiple: true,
    });
    if (selected) {
      buildOptions.value.fontpaths = Array.isArray(selected)
        ? selected
        : [selected];
    }
  }

  async function pickBuildOutputDir() {
    const selected = await open({
      directory: true,
    });
    if (selected && typeof selected === 'string') {
      buildOptions.value.output = selected;
    }
  }

  async function handleRunProcess() {
    processOptions.value.inputs = inputs.value
      .split('|')
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    processOptions.value.fontpaths = fonts.value
      .split('|')
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    progress.value = 0;


    if (processOptions.value.inputs.length === 0) {
      toast.add({
        severity: 'error',
        summary: 'Error',
        detail: 'Please select input files',
        life: 3000,
      });
      return;
    }
    isProcessing.value = true;
    logs.value = [];
    addLog('Starting font embedding...');
    try {
      await commands.runProcess(processOptions.value);
      toast.add({
        severity: 'success',
        summary: 'Success',
        detail: 'Processing completed',
        life: 3000,
      });
      addLog('Successfully completed.');
    } catch (e: unknown) {
      addLog(`Error: ${e}`);
      toast.add({
        severity: 'error',
        summary: 'Processing Failed',
        detail: String(e),
        life: 5000,
      });
    } finally {
      isProcessing.value = false;
    }
  }

  async function handleRunBuild() {
    buildOptions.value.fontpaths = fonts.value
      .split('|')
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    if (buildOptions.value.fontpaths.length === 0) {
      toast.add({
        severity: 'error',
        summary: 'Error',
        detail: 'Please select font directories',
        life: 3000,
      });
      return;
    }
    isProcessing.value = true;
    addLog('Starting index building...');
    try {
      await commands.runBuild(buildOptions.value);
      toast.add({
        severity: 'success',
        summary: 'Success',
        detail: 'Index built successfully',
        life: 3000,
      });
      addLog('Index build complete.');
    } catch (e: unknown) {
      addLog(`Error: ${e}`);
      toast.add({
        severity: 'error',
        summary: 'Build Failed',
        detail: String(e),
        life: 5000,
      });
    } finally {
      isProcessing.value = false;
    }
  }
</script>

<template>
  <div class="p-4 h-screen flex flex-col gap-4">
    <Toast />
    <div class="flex items-center gap-2 mb-2">
      <h1 class="text-2xl font-bold m-0">Assfonts GUI</h1>
      <span class="text-sm">v0.1.0</span>
    </div>

    <Tabs v-model:value="activeTab">
      <TabList>
        <Tab value="0">Process (Embed Fonts)</Tab>
        <Tab value="1">Build Font Index</Tab>
      </TabList>
      <TabPanels>
        <TabPanel value="0">
          <div class="flex flex-col gap-4">
            <div class="flex flex-col gap-2">
              <label class="font-bold">Input Subtitles</label>
              <div class="flex gap-2">
                <InputText
                  v-model="inputs"
                  class="flex-1"
                  placeholder="Select .ass files..."
                />
                <Button
                  label="Browse"
                  icon="pi pi-file"
                  @click="pickFiles"
                  :disabled="isProcessing"
                />
                <Button @click="pickDirs" :disabled="isProcessing" />
              </div>
            </div>

            <div class="grid grid-cols-2 gap-4">
              <div class="flex flex-col gap-2">
                <label class="font-bold">Output Directory</label>
                <div class="flex gap-2">
                  <InputText
                    v-model="processOptions.output"
                    class="flex-1"
                    placeholder="Current directory"
                  />
                  <Button
                    icon="pi pi-folder-open"
                    @click="pickOutputDir"
                    :disabled="isProcessing"
                  />
                </div>
              </div>
              <div class="flex flex-col gap-2">
                <label class="font-bold">Database Path (Index)</label>
                <div class="flex gap-2">
                  <InputText
                    v-model="processOptions.dbpath"
                    class="flex-1"
                    placeholder="Directory of fonts.index.json"
                  />
                  <Button
                    icon="pi pi-search"
                    @click="pickDbDir"
                    :disabled="isProcessing"
                  />
                </div>
              </div>
            </div>

            <div class="flex flex-col gap-2">
              <label class="font-bold">Additional Font Paths (Optional)</label>
              <div class="flex gap-2">
                <InputText
                  v-model="fonts"
                  class="flex-1"
                  placeholder="Search these folders for fonts..."
                />
                <Button
                  label="Browse"
                  icon="pi pi-plus"
                  @click="pickFontDirs"
                  :disabled="isProcessing"
                />
              </div>
            </div>

            <div class="flex flex-wrap gap-4 p-4">
              <div class="flex items-center gap-2">
                <Checkbox
                  v-model="processOptions.strict"
                  :binary="true"
                  inputId="strict"
                />
                <label for="strict">Strict Mode</label>
              </div>
              <div class="flex items-center gap-2">
                <Checkbox
                  v-model="processOptions.report"
                  :binary="true"
                  inputId="report"
                />
                <label for="report">Generate Report</label>
              </div>
              <div class="flex items-center gap-2">
                <Checkbox
                  v-model="processOptions.force"
                  :binary="true"
                  inputId="force"
                />
                <label for="force">Overwrite Existing</label>
              </div>
              <div class="flex items-center gap-2">
                <Checkbox
                  v-model="processOptions.allow_missing_sample"
                  :binary="true"
                  inputId="missing_sample"
                />
                <label for="missing_sample">Allow Missing Sample</label>
              </div>
              <div class="flex items-center gap-2">
                <Checkbox
                  v-model="processOptions.allow_missing_fonts"
                  :binary="true"
                  inputId="missing_fonts"
                />
                <label for="missing_fonts">Allow Missing Fonts</label>
              </div>
            </div>

            <div class="flex justify-center mt-2 flex-col gap-2">
              <Button
                label="Start Processing"
                icon="pi pi-play"
                size="large"
                @click="handleRunProcess"
                :loading="isProcessing"
              />
              <ProgressBar
                v-if="isProcessing"
                :value="progress"
                class="w-full h-2"
              />
            </div>
          </div>
        </TabPanel>

        <TabPanel value="1">
          <div class="flex flex-col gap-4">
            <div class="flex flex-col gap-2">
              <label class="font-bold">Font Directories to Scan</label>
              <div class="flex gap-2">
                <InputText
                  v-model="fonts"
                  class="flex-1"
                  placeholder="Select folders to search for fonts..."
                />
                <Button
                  label="Browse"
                  icon="pi pi-folder-plus"
                  @click="pickBuildFontDirs"
                  :disabled="isProcessing"
                />
              </div>
            </div>

            <div class="flex flex-col gap-2">
              <label class="font-bold"
                >Output Directory for fonts.index.json</label
              >
              <div class="flex gap-2">
                <InputText
                  v-model="buildOptions.output"
                  class="flex-1"
                  placeholder="Current directory"
                />
                <Button
                  icon="pi pi-folder-open"
                  @click="pickBuildOutputDir"
                  :disabled="isProcessing"
                />
              </div>
            </div>

            <div class="flex justify-center mt-2">
              <Button
                label="Build Font Index"
                icon="pi pi-hammer"
                severity="secondary"
                size="large"
                @click="handleRunBuild"
                :loading="isProcessing"
              />
            </div>
          </div>
        </TabPanel>
      </TabPanels>
    </Tabs>

    <div class="flex-1 flex flex-col gap-2 min-h-0">
      <div class="flex items-center justify-between">
        <label class="font-bold">Logs</label>
        <Button
          icon="pi pi-trash"
          text
          rounded
          size="small"
          @click="logs = []"
        />
      </div>
      <div class="flex-1 p-2 font-mono text-sm min-h-[200px] overflow-y-auto rounded border">
        <div v-for="(log, idx) in logs" :key="idx">{{ log }}</div>
        <div v-if="logs.length === 0" class="italic">No logs yet.</div>
      </div>
    </div>
  </div>
</template>

<style>
  /* Adjustments for fixed height with flex */
  body,
  html,
  #app {
    margin: 0;
    padding: 0;
    height: 100%;
  }
</style>
