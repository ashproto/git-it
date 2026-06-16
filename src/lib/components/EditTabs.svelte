<script lang="ts">
  import { appState } from "../store.svelte";
  import { parseISO, buildLocalDate, formatCommitDate } from "../dates";
  import type { EditMode } from "../types";
  import CollapsiblePanel from "./CollapsiblePanel.svelte";

  // When bare, render without the CollapsiblePanel chrome (the mode-tabs become a
  // top row) so this nests inside a parent panel (InlineEditCommit).
  let { bare = false }: { bare?: boolean } = $props();

  let mode = $state<EditMode>("offset");

  // Offset state
  let sign = $state<"+" | "-">("+");
  let days = $state(0);
  let hours = $state(0);
  let minutes = $state(0);
  let seconds = $state(0);

  // Exact state
  const now = new Date();
  let year = $state(now.getFullYear());
  let month = $state(now.getMonth() + 1);
  let day = $state(now.getDate());
  let hour = $state(now.getHours());
  let minute = $state(now.getMinutes());
  let second = $state(now.getSeconds());

  // Compress state
  let sYear = $state(now.getFullYear());
  let sMonth = $state(now.getMonth() + 1);
  let sDay = $state(now.getDate());
  let sHour = $state(now.getHours());
  let sMinute = $state(now.getMinutes());
  let sSecond = $state(now.getSeconds());
  let eYear = $state(now.getFullYear());
  let eMonth = $state(now.getMonth() + 1);
  let eDay = $state(now.getDate());
  let eHour = $state(now.getHours());
  let eMinute = $state(now.getMinutes());
  let eSecond = $state(now.getSeconds());

  // Live read-only previews — reflect both the entered fields and the user's
  // display preferences (appState.dateFormat). buildLocalDate returns null for
  // incomplete/impossible input, so the preview shows a clear placeholder instead
  // of a silently-wrong date (e.g. a cleared year would otherwise read 1900).
  const INCOMPLETE = "— enter a complete, valid date";
  const exactPreview = $derived.by(() => {
    const d = buildLocalDate(year, month, day, hour, minute, second);
    return d ? formatCommitDate(d, appState.dateFormat) : INCOMPLETE;
  });
  const compressOldestPreview = $derived.by(() => {
    const d = buildLocalDate(sYear, sMonth, sDay, sHour, sMinute, sSecond);
    return d ? formatCommitDate(d, appState.dateFormat) : INCOMPLETE;
  });
  const compressLatestPreview = $derived.by(() => {
    const d = buildLocalDate(eYear, eMonth, eDay, eHour, eMinute, eSecond);
    return d ? formatCommitDate(d, appState.dateFormat) : INCOMPLETE;
  });

  function needsSelection(): string[] | null {
    const shas = Array.from(appState.selected);
    if (shas.length === 0) {
      appState.status = "Select one or more commits first.";
      return null;
    }
    return shas;
  }

  function previewOffset() {
    const shas = needsSelection();
    if (!shas) return;
    // A cleared number field binds as null; treat an empty unit as 0 (a delta of
    // "no days" means 0 days) so the offset is well-defined rather than NaN.
    const n = (v: number) => (Number.isFinite(v) ? v : 0);
    const deltaMs =
      (sign === "+" ? 1 : -1) *
      ((n(days) * 86400 + n(hours) * 3600 + n(minutes) * 60 + n(seconds)) * 1000);
    let count = 0;
    for (const sha of shas) {
      const commit = appState.commits.find((c) => c.sha === sha);
      if (!commit) continue;
      const base = parseISO(commit.committer_date);
      if (!base) continue;
      appState.setNewDate(sha, new Date(base.getTime() + deltaMs));
      count++;
    }
    appState.status = `Previewed offset on ${count} commit(s).`;
  }

  function previewExact() {
    const shas = needsSelection();
    if (!shas) return;
    const d = buildLocalDate(year, month, day, hour, minute, second);
    if (!d) {
      appState.status = "Enter a complete, valid date.";
      return;
    }
    for (const sha of shas) appState.setNewDate(sha, d);
    appState.status = `Previewed exact time for ${shas.length} commit(s).`;
  }

  function previewCompress() {
    const shas = needsSelection();
    if (!shas) return;
    if (shas.length < 2) {
      appState.status = "Compress needs at least 2 selected commits.";
      return;
    }
    const start = buildLocalDate(sYear, sMonth, sDay, sHour, sMinute, sSecond);
    const end = buildLocalDate(eYear, eMonth, eDay, eHour, eMinute, eSecond);
    if (!start || !end) {
      appState.status = "Enter a complete, valid date.";
      return;
    }
    if (end.getTime() <= start.getTime()) {
      appState.status = "Compress: end must be after start.";
      return;
    }

    // Sort selected by current committer date ascending, then map proportionally
    // from current range to the new [start, end] range.
    const items = shas
      .map((sha) => {
        const c = appState.commits.find((x) => x.sha === sha);
        const dt = c ? parseISO(c.committer_date) : null;
        return dt ? { sha, t: dt.getTime() } : null;
      })
      .filter((x): x is { sha: string; t: number } => !!x)
      .sort((a, b) => a.t - b.t);

    if (items.length < 2) {
      appState.status = "Compress: couldn't read dates for the selected commits.";
      return;
    }

    const minT = items[0].t;
    const maxT = items[items.length - 1].t;
    const span = Math.max(1, maxT - minT);
    const newSpan = end.getTime() - start.getTime();

    for (const item of items) {
      const ratio = (item.t - minT) / span;
      const newT = start.getTime() + ratio * newSpan;
      appState.setNewDate(item.sha, new Date(newT));
    }
    appState.status = `Compressed ${items.length} commit(s) into ${formatCommitDate(start, appState.dateFormat)} → ${formatCommitDate(end, appState.dateFormat)}.`;
  }

  function setNowExact() {
    const n = new Date();
    year = n.getFullYear();
    month = n.getMonth() + 1;
    day = n.getDate();
    hour = n.getHours();
    minute = n.getMinutes();
    second = n.getSeconds();
  }

  function copyExactFromSelected() {
    const shas = Array.from(appState.selected);
    if (shas.length === 0) return;
    const commit = appState.commits.find((c) => c.sha === shas[0]);
    if (!commit) return;
    const d = parseISO(commit.committer_date);
    if (!d) return;
    year = d.getFullYear();
    month = d.getMonth() + 1;
    day = d.getDate();
    hour = d.getHours();
    minute = d.getMinutes();
    second = d.getSeconds();
  }

  function initCompressFromSelection() {
    const shas = Array.from(appState.selected);
    if (shas.length < 2) return;
    const dates = shas
      .map((s) => appState.commits.find((c) => c.sha === s))
      .map((c) => (c ? parseISO(c.committer_date) : null))
      .filter((d): d is Date => !!d)
      .sort((a, b) => a.getTime() - b.getTime());
    if (dates.length < 2) return;
    const s = dates[0];
    const e = dates[dates.length - 1];
    sYear = s.getFullYear();
    sMonth = s.getMonth() + 1;
    sDay = s.getDate();
    sHour = s.getHours();
    sMinute = s.getMinutes();
    sSecond = s.getSeconds();
    eYear = e.getFullYear();
    eMonth = e.getMonth() + 1;
    eDay = e.getDate();
    eHour = e.getHours();
    eMinute = e.getMinutes();
    eSecond = e.getSeconds();
  }

  function clearNewDatesSel() {
    const shas = Array.from(appState.selected);
    appState.clearNewDates(shas.length ? shas : undefined);
    appState.status = `Cleared ${shas.length || "all"} new date(s).`;
  }
</script>

{#snippet modeTabs()}
  <div class="mode-tabs">
    <button
      type="button"
      class:active={mode === "offset"}
      onclick={() => (mode = "offset")}>Offset</button
    >
    <button
      type="button"
      class:active={mode === "exact"}
      onclick={() => (mode = "exact")}>Exact</button
    >
    <button
      type="button"
      class:active={mode === "compress"}
      onclick={() => (mode = "compress")}>Compress</button
    >
  </div>
{/snippet}

{#snippet body()}
  {#if mode === "offset"}
    <div class="row">
      <label class="check">
        <input type="radio" bind:group={sign} value="+" /> +
      </label>
      <label class="check">
        <input type="radio" bind:group={sign} value="-" /> −
      </label>
      <span class="spacer"></span>
      <label class="inline">Days <input type="number" min="0" max="365" bind:value={days} class="num" /></label>
      <label class="inline">Hours <input type="number" min="0" max="23" bind:value={hours} class="num" /></label>
      <label class="inline">Min <input type="number" min="0" max="59" bind:value={minutes} class="num" /></label>
      <label class="inline">Sec <input type="number" min="0" max="59" bind:value={seconds} class="num" /></label>
      <button type="button" class="primary" onclick={previewOffset}>Preview</button>
    </div>
  {:else if mode === "exact"}
    <div class="row">
      <label class="inline">Y <input type="number" min="1970" max="2100" bind:value={year} class="num" style="width:5rem" /></label>
      <label class="inline">M <input type="number" min="1" max="12" bind:value={month} class="num" /></label>
      <label class="inline">D <input type="number" min="1" max="31" bind:value={day} class="num" /></label>
      <label class="inline">h <input type="number" min="0" max="23" bind:value={hour} class="num" /></label>
      <label class="inline">m <input type="number" min="0" max="59" bind:value={minute} class="num" /></label>
      <label class="inline">s <input type="number" min="0" max="59" bind:value={second} class="num" /></label>
      <button type="button" onclick={setNowExact}>Now</button>
      <button type="button" onclick={copyExactFromSelected}>Copy from selected</button>
      <button type="button" class="primary" onclick={previewExact}>Preview</button>
    </div>
    <p class="preview-line">Preview: <span class="mono">{exactPreview}</span></p>
  {:else}
    <!-- Latest on top, Oldest below — matches the commit list (latest-first). -->
    <div class="row">
      <span class="lbl-w">Latest</span>
      <label class="inline">Y <input type="number" min="1970" max="2100" bind:value={eYear} class="num" style="width:5rem" /></label>
      <label class="inline">M <input type="number" min="1" max="12" bind:value={eMonth} class="num" /></label>
      <label class="inline">D <input type="number" min="1" max="31" bind:value={eDay} class="num" /></label>
      <label class="inline">h <input type="number" min="0" max="23" bind:value={eHour} class="num" /></label>
      <label class="inline">m <input type="number" min="0" max="59" bind:value={eMinute} class="num" /></label>
      <label class="inline">s <input type="number" min="0" max="59" bind:value={eSecond} class="num" /></label>
    </div>
    <p class="preview-line">Latest: <span class="mono">{compressLatestPreview}</span></p>
    <div class="row">
      <span class="lbl-w">Oldest</span>
      <label class="inline">Y <input type="number" min="1970" max="2100" bind:value={sYear} class="num" style="width:5rem" /></label>
      <label class="inline">M <input type="number" min="1" max="12" bind:value={sMonth} class="num" /></label>
      <label class="inline">D <input type="number" min="1" max="31" bind:value={sDay} class="num" /></label>
      <label class="inline">h <input type="number" min="0" max="23" bind:value={sHour} class="num" /></label>
      <label class="inline">m <input type="number" min="0" max="59" bind:value={sMinute} class="num" /></label>
      <label class="inline">s <input type="number" min="0" max="59" bind:value={sSecond} class="num" /></label>
    </div>
    <p class="preview-line">Oldest: <span class="mono">{compressOldestPreview}</span></p>
    <div class="row actions">
      <button type="button" onclick={initCompressFromSelection}>Init from selection</button>
      <button type="button" class="primary" onclick={previewCompress}>Preview</button>
    </div>
  {/if}

  <div class="row" style="margin-top:8px">
    <button type="button" onclick={clearNewDatesSel}>Clear new dates (selected)</button>
  </div>
{/snippet}

{#if bare}
  <div class="edit-bare">
    {@render modeTabs()}
    {@render body()}
  </div>
{:else}
  <CollapsiblePanel title="Edit">
    {#snippet headerActions()}
      {@render modeTabs()}
    {/snippet}
    {@render body()}
  </CollapsiblePanel>
{/if}

<style>
  /* Bare layout: mode-tabs row above the body, no panel chrome. The tabs sit
     left-aligned (the panel header right-aligns them via .actions). */
  .edit-bare {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .edit-bare .mode-tabs {
    align-self: flex-start;
  }
  .mode-tabs {
    display: flex;
    gap: 2px;
    background: var(--input-bg);
    border-radius: 6px;
    padding: 2px;
  }
  .mode-tabs button {
    padding: 4px 10px;
    border-radius: 4px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .mode-tabs button.active {
    background: var(--btn-bg);
    color: var(--text);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
    flex-wrap: wrap;
  }
  .row:first-of-type {
    margin-top: 0;
  }
  .row.actions {
    justify-content: flex-end;
  }
  label.check {
    display: flex;
    gap: 4px;
    align-items: center;
    color: var(--text);
    font-size: 12.5px;
  }
  .lbl-w {
    min-width: 56px;
    font-size: 12.5px;
    color: var(--text-muted);
    font-weight: 500;
  }
  label.inline {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .num {
    width: 4rem;
    padding: 4px 6px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--input-bg);
    color: var(--text);
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .spacer {
    width: 6px;
  }
  .preview-line {
    margin: 4px 0 0;
    font-size: 11.5px;
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .preview-line .mono {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    color: var(--text);
  }
  button {
    padding: 5px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  button:hover {
    background: var(--btn-hover);
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  button.primary:hover {
    background: var(--accent-hover);
  }
</style>
