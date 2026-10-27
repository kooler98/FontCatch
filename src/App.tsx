import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { RecentCatches, message } from "./components/RecentCatches";
import { NativeFontDetail } from "./components/NativeFontDetail";
import { NativeFontSample } from "./components/NativeFontSample";
import { preferredFont } from "./lib/font-preview";
import { version } from "../package.json";
import {
  emptySnapshot,
  familiesFromSnapshot,
  type Snapshot,
  type CatchJob,
} from "./lib/native";
import { readSnapshot, installFont, ignoreFont } from "./lib/bridge";
import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import logo from "./assets/brand/fontcatch.svg";
import { Icon } from "./components/Icon";
import { Search } from "./components/beautiful-ui/Search";
import { FilterChips } from "./components/beautiful-ui/FilterChips";
import { TaskRow } from "./components/beautiful-ui/TaskRows";
import { ApprovalCard } from "./components/beautiful-ui/ApprovalCard";
import { LoadingState } from "./components/beautiful-ui/LoadingState";
import {
  native,
  readSettings,
  saveSettings,
  chooseFolder,
  openLink,
  hidePanel,
} from "./lib/bridge";
import {
  defaultSettings,
  type Family,
  type Settings,
  type Tab,
} from "./lib/types";
import { demoFamilies, filterFamilies } from "./lib/library";
import fr from "./locales/fr.json";
import en from "./locales/en.json";
import config from "./config.json";

export default function App() {
  const [settings, setSettings] = useState<Settings>(defaultSettings);
  const [, refreshRelativeDates] = useState(0);
  const [snapshot, setSnapshot] = useState<Snapshot>(emptySnapshot);
  const [actionId, setActionId] = useState<string | null>(null);
  const [notificationPermission, setNotificationPermission] = useState<
    "default" | "granted" | "denied"
  >("default");
  const [ready, setReady] = useState(false);
  const [tab, setTab] = useState<Tab>("recent");
  const [demo, setDemo] = useState(!native);
  const [families, setFamilies] = useState<Family[]>(demoFamilies);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<"all" | "variable" | "static">("all");
  const [source, setSource] = useState("");
  const [detail, setDetail] = useState<Family | null>(null);
  const [about, setAbout] = useState(false);
  const [sample, setSample] = useState("");
  const [fontSize, setFontSize] = useState(30);
  const [busy, setBusy] = useState(false);
  const [autoStart, setAutoStart] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [installing, setInstalling] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lock = useRef(false);
  const t = settings.language === "fr" ? fr : en;
  const data = useMemo(
    () => (native ? familiesFromSnapshot(snapshot) : demo ? families : []),
    [snapshot.installed, demo, families],
  );
  const results = useMemo(
    () => filterFamilies(data, query, kind, source),
    [data, query, kind, source],
  );
  const pending = data.find((f) => f.status === "pending");
  const init = async () => {
    setError("");
    try {
      setSettings(await readSettings());
      if (native) {
        setAutoStart(await isEnabled());
        setSnapshot(await readSnapshot());
        setNotificationPermission(
          (await isPermissionGranted()) ? "granted" : "default",
        );
      }
      setReady(true);
    } catch (e) {
      setError(String(e));
    }
  };
  useEffect(() => {
    void init();
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);
  useEffect(() => {
    const timer = setInterval(() => {
      if (!document.hidden) refreshRelativeDates((n) => n + 1);
    }, 60000);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => {
    document.documentElement.lang = settings.language;
  }, [settings.language]);
  useEffect(() => {
    if (!notice) return;
    const id = setTimeout(() => setNotice(""), 2200);
    return () => clearTimeout(id);
  }, [notice]);
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const attach = async () => {
      for (const promise of [
        listen<Snapshot>("library-changed", (event) =>
          setSnapshot(event.payload),
        ),
        listen<Tab>("navigate", (event) => {
          setTab(event.payload);
          setDetail(null);
          setAbout(false);
        }),
        listen<Settings>("settings-changed", (event) =>
          setSettings(event.payload),
        ),
      ]) {
        const off = await promise;
        if (disposed) off();
        else unlisteners.push(off);
      }
    };
    void attach().catch((e) => setError(String(e)));
    return () => {
      disposed = true;
      unlisteners.forEach((off) => off());
    };
  }, []);
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (document.querySelector("dialog[open]")) return;
        if (detail) setDetail(null);
        else if (about) setAbout(false);
        else void hidePanel().catch((e) => setError(String(e)));
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [detail, about]);
  useEffect(() => {
    if (
      native &&
      detail &&
      !snapshot.installed.some((f) => f.face.family === detail.name)
    )
      setDetail(null);
  }, [snapshot, detail]);
  async function update(patch: Partial<Settings>) {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      setSettings(await saveSettings({ ...settings, ...patch }));
      setNotice(t.saved);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      lock.current = false;
    }
  }
  async function addFolder() {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    try {
      const folder = await chooseFolder();
      if (folder) {
        setSettings(
          await saveSettings({
            ...settings,
            folders: [...new Set([...settings.folders, folder])],
          }),
        );
        setNotice(t.saved);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      lock.current = false;
    }
  }
  async function toggleAutostart() {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    try {
      await (autoStart ? disable() : enable());
      setAutoStart(await isEnabled());
      setNotice(t.saved);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      lock.current = false;
    }
  }
  async function actOnFont(job: CatchJob) {
    if (actionId) return;
    setActionId(job.id);
    setError("");
    try {
      setSnapshot(await installFont(job.id, job.requiresUpdate));
    } catch (e) {
      setError(message(String(e), t));
    } finally {
      setActionId(null);
    }
  }
  async function ignoreCatch(id: string) {
    if (actionId) return;
    setActionId(id);
    setError("");
    try {
      setSnapshot(await ignoreFont(id));
    } catch (e) {
      setError(message(String(e), t));
    } finally {
      setActionId(null);
    }
  }
  async function allowNotifications() {
    try {
      setNotificationPermission(await requestPermission());
    } catch (e) {
      setError(String(e));
    }
  }
  function installDemo() {
    if (!demo || !pending || installing) return;
    const id = pending.id;
    setInstalling(true);
    timer.current = setTimeout(() => {
      setFamilies((prev) =>
        prev.map((f) => (f.id === id ? { ...f, status: "installed" } : f)),
      );
      setInstalling(false);
    }, 900);
  }
  function changeTab(next: Tab) {
    setTab(next);
    setDetail(null);
    setAbout(false);
  }
  function resetDemo(next: boolean) {
    if (timer.current) clearTimeout(timer.current);
    setInstalling(false);
    setDemo(next);
    setFamilies(demoFamilies);
    setDetail(null);
    setAbout(false);
    setQuery("");
    setKind("all");
    setSource("");
  }
  const link = (key: keyof typeof config) =>
    void openLink(key).catch((e) => setError(String(e)));
  const tabs: Tab[] = ["recent", "library", "settings"];
  const panel = (
    <div className="panel" aria-label="Fontcatch">
      <header className="panel-header">
        <img className="header-logo" src={logo} alt="Fontcatch" />
        <div className="header-right">
          <span className="version-pill">{version}</span>
          {native && (
            <button
              className="icon-button"
              onClick={() => void hidePanel().catch((e) => setError(String(e)))}
              aria-label={t.close}
            >
              <Icon name="close" size={14} />
            </button>
          )}
        </div>
      </header>
      <nav className="tabs" aria-label="Navigation">
        <div role="tablist" className="tablist">
          {tabs.map((item, i) => (
            <button
              key={item}
              role="tab"
              id={`tab-${item}`}
              aria-selected={tab === item}
              aria-controls="content"
              tabIndex={tab === item ? 0 : -1}
              className={tab === item ? "active" : ""}
              onClick={() => changeTab(item)}
              onKeyDown={(event) => {
                if (
                  ["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)
                ) {
                  event.preventDefault();
                  const next =
                    event.key === "Home"
                      ? 0
                      : event.key === "End"
                        ? 2
                        : (i + (event.key === "ArrowRight" ? 1 : 2)) % 3;
                  changeTab(tabs[next]);
                  document.getElementById(`tab-${tabs[next]}`)?.focus();
                }
              }}
            >
              <Icon name={item} />
              {t[item]}
              {item === "recent" &&
                (pending ||
                  (native &&
                    snapshot.recent.some((j) => j.status === "pending"))) && (
                  <span className="tab-dot" />
                )}
            </button>
          ))}
        </div>
      </nav>
      {demo && <div className="demo-banner">{t.demoNote}</div>}
      {error && (
        <div className="error-banner" role="alert">
          <strong>{t.error}</strong>
          <span>{error}</span>
          {!ready && <button onClick={() => void init()}>{t.retry}</button>}
          <button aria-label={t.close} onClick={() => setError("")}>
            ×
          </button>
        </div>
      )}
      <main
        id="content"
        className="panel-content"
        role="tabpanel"
        aria-labelledby={`tab-${tab}`}
        tabIndex={0}
        key={`${tab}-${detail?.id ?? ""}-${about}`}
      >
        {!ready ? (
          <LoadingState label="Fontcatch…" />
        ) : about ? (
          <div className="about-view">
            <button className="back-button" onClick={() => setAbout(false)}>
              <Icon name="back" />
              {t.back}
            </button>
            <img src={logo} alt="Fontcatch" className="about-logo" />
            <h2>{t.aboutBody}</h2>
            <p>{t.aboutPrivacy}</p>
            <span className="version-label">
              {t.version} {version}
            </span>
            {config.websiteUrl && (
              <button
                className="setting-link"
                disabled={!config.websiteUrl}
                onClick={() => link("websiteUrl")}
              >
                {t.website}
                <Icon name="arrow" />
              </button>
            )}
            {config.coffeeUrl && (
              <button
                className="primary-button"
                disabled={!config.coffeeUrl}
                onClick={() => link("coffeeUrl")}
              >
                <Icon name="coffee" />
                {t.support}
              </button>
            )}
          </div>
        ) : detail && native ? (
          <NativeFontDetail
            key={detail.id}
            family={detail.name}
            fonts={snapshot.installed.filter(
              (f) => f.face.family === detail.name,
            )}
            t={t}
            language={settings.language}
            onBack={() => setDetail(null)}
            onSnapshot={setSnapshot}
            onNotice={setNotice}
          />
        ) : detail ? (
          <div className="detail-view">
            <button className="back-button" onClick={() => setDetail(null)}>
              <Icon name="back" />
              {t.back}
            </button>
            <div className="section-title">
              <h1>{detail.name}</h1>
              <span>{detail.variable ? t.variable : t.static}</span>
            </div>
            <p className="subtle">{detail.foundry}</p>
            <label className="preview-label" htmlFor="font-preview">
              {t.demoFont}
            </label>
            <textarea
              id="font-preview"
              aria-label={t.previewText}
              value={sample}
              onChange={(e) => setSample(e.target.value)}
              style={{ fontSize, fontFamily: detail.preview }}
              spellCheck={false}
            />
            <label className="slider-label">
              {t.size}
              <input
                type="range"
                min="16"
                max="64"
                value={fontSize}
                onChange={(e) => setFontSize(Number(e.target.value))}
              />
              <span>{fontSize}px</span>
            </label>
            <div className="styles-list">
              {detail.styles.map((style) => (
                <span key={style}>{style}</span>
              ))}
            </div>
            <button
              className="secondary-button"
              onClick={() =>
                void navigator.clipboard
                  .writeText(detail.name)
                  .then(() => setNotice(t.copied))
                  .catch((e) => setError(String(e)))
              }
            >
              <Icon name="copy" />
              {t.copyName}
            </button>
            <p className="license-label">{t.licenseUnknown}</p>
            <p className="subtle">{t.previewOnly}</p>
          </div>
        ) : tab === "recent" ? (
          <>
            {native &&
            (snapshot.recent.length > 0 ||
              snapshot.processing ||
              snapshot.watcherErrors.length > 0) ? (
              <RecentCatches
                snapshot={snapshot}
                t={t}
                language={settings.language}
                busy={actionId}
                onInstall={(job) => void actOnFont(job)}
                onIgnore={(id) => void ignoreCatch(id)}
              />
            ) : data.length ? (
              <>
                <div className="section-title">
                  <div>
                    <h1>{t.recentTitle}</h1>
                  </div>
                </div>
                {pending &&
                  (installing ? (
                    <LoadingState label={t.extracting} />
                  ) : (
                    <ApprovalCard
                      title={t.newFont}
                      installLabel={t.install}
                      ignoreLabel={t.ignore}
                      onInstall={installDemo}
                      onIgnore={() =>
                        setFamilies((prev) =>
                          prev.map((f) =>
                            f.id === pending.id
                              ? { ...f, status: "ignored" }
                              : f,
                          ),
                        )
                      }
                    >
                      <div className="approval-font">
                        <span className="approval-aa">Ag</span>
                        <div>
                          <h2>{pending.name}</h2>
                          <p>
                            {pending.styles.length}{" "}
                            {pending.styles.length === 1 ? t.style : t.styles} ·{" "}
                            {t.variable}
                          </p>
                          <span>{pending.source}</span>
                        </div>
                      </div>
                    </ApprovalCard>
                  ))}
                <div className="list-label">
                  {t.today}
                  <span>
                    {data.filter((f) => f.status !== "pending").length}
                  </span>
                </div>
                <div className="task-list">
                  {data
                    .filter((f) => f.status !== "pending")
                    .map((family, index) => (
                      <TaskRow
                        key={family.id}
                        name={family.name}
                        meta={`${family.styles.length} ${family.styles.length === 1 ? t.style : t.styles} · ${family.source}`}
                        status={t[family.status]}
                        installed={family.status === "installed"}
                        index={index}
                      >
                        <p>{family.styles.join(" · ")}</p>
                        <p>{family.foundry}</p>
                      </TaskRow>
                    ))}
                </div>
              </>
            ) : (
              <div className="empty-state">
                <h1>{t.emptyTitle}</h1>
                <p>{t.emptyBody}</p>
                <span className="empty-file-types">
                  .zip <span>·</span> .ttf <span>·</span> .otf <span>·</span>{" "}
                  .ttc
                </span>
              </div>
            )}
          </>
        ) : tab === "library" ? (
          <>
            <div className="section-title">
              <h1>{t.library}</h1>
              <span>
                {data.filter((f) => f.status === "installed").length}{" "}
                {t.families}
              </span>
            </div>
            <Search
              value={query}
              onChange={setQuery}
              placeholder={t.search}
              clearLabel={t.clear}
            />
            <div className="filters">
              <FilterChips
                value={kind}
                options={[
                  { value: "all", label: t.all },
                  { value: "variable", label: t.variable },
                  { value: "static", label: t.static },
                ]}
                onChange={setKind}
                label={t.typeFilter}
              />
              <select
                className="source-filter"
                aria-label={t.source}
                value={source}
                onChange={(e) => setSource(e.target.value)}
              >
                <option value="">{t.allSources}</option>
                {[
                  ...new Set(
                    data
                      .filter((f) => f.status === "installed")
                      .flatMap((f) => f.sources ?? [f.source]),
                  ),
                ].map((s) => (
                  <option key={s}>{s}</option>
                ))}
              </select>
            </div>
            {results.length ? (
              <div className="library-list">
                {results.map((family) => (
                  <button
                    className="library-card"
                    key={family.id}
                    onClick={() => {
                      setDetail(family);
                      setSample(t.sample);
                    }}
                  >
                    <div className="library-card-head">
                      <strong>{family.name}</strong>
                      <span>
                        {family.variable && family.hasStatic
                          ? t.both
                          : family.variable
                            ? t.variable
                            : t.static}
                        <Icon name="chevron" size={12} />
                      </span>
                    </div>
                    {native ? (
                      <NativeFontSample
                        font={preferredFont(
                          snapshot.installed.filter(
                            (f) => f.face.family === family.name,
                          ),
                        )}
                        loading={t.loadingPreview}
                        unavailable={t.previewUnavailable}
                      />
                    ) : (
                      <span
                        className="type-sample"
                        style={{ fontFamily: family.preview }}
                      >
                        Aa Bb Cc Dd
                      </span>
                    )}
                    <div className="library-card-bottom">
                      <span>
                        {family.styles.length}{" "}
                        {family.styles.length === 1 ? t.style : t.styles}
                      </span>
                      <span>{family.foundry}</span>
                    </div>
                  </button>
                ))}
              </div>
            ) : (
              <div className="library-empty">
                <Icon name="library" size={26} />
                <h2>{data.length ? t.noResults : t.libraryEmpty}</h2>
                <p>{data.length ? t.noResultsHint : t.libraryEmptyHint}</p>
              </div>
            )}
          </>
        ) : (
          <div className="settings-view">
            <div className="section-title">
              <h1>{t.settings}</h1>
            </div>
            <fieldset disabled={busy}>
              <legend>{t.behavior}</legend>
              <label className="setting-label">{t.mode}</label>
              <div className="segmented">
                {(["ask", "auto"] as const).map((mode) => (
                  <button
                    key={mode}
                    aria-pressed={settings.mode === mode}
                    className={settings.mode === mode ? "selected" : ""}
                    onClick={() => void update({ mode })}
                  >
                    {t[mode]}
                    {settings.mode === mode && <span aria-hidden>✓</span>}
                  </button>
                ))}
              </div>
              <p className="setting-hint">
                {settings.mode === "ask" ? t.askHint : t.autoHint}
              </p>
              <label className="select-row">
                <span>{t.preference}</span>
                <select
                  aria-label={t.preference}
                  value={settings.fontPreference}
                  onChange={(e) =>
                    void update({
                      fontPreference: e.target
                        .value as Settings["fontPreference"],
                    })
                  }
                >
                  <option value="variable">{t.variableOnly}</option>
                  <option value="static">{t.staticOnly}</option>
                  <option value="both">{t.both}</option>
                </select>
              </label>
              <div className="setting-row">
                <div>
                  <label id="trash-label">{t.trash}</label>
                  <p>{t.trashHint}</p>
                </div>
                <button
                  className="switch"
                  role="switch"
                  aria-checked={settings.trashAfterInstall}
                  aria-labelledby="trash-label"
                  onClick={() =>
                    void update({
                      trashAfterInstall: !settings.trashAfterInstall,
                    })
                  }
                >
                  <span />
                </button>
              </div>
            </fieldset>
            <fieldset disabled={busy}>
              <legend>{t.folders}</legend>
              {settings.folders.length ? (
                settings.folders.map((folder) => (
                  <div className="folder-row" key={folder}>
                    <Icon name="folder" />
                    <span title={folder}>{folder}</span>
                    <button
                      className="icon-button"
                      aria-label={`${t.removeFolder} ${folder}`}
                      onClick={() =>
                        void update({
                          folders: settings.folders.filter((f) => f !== folder),
                        })
                      }
                    >
                      <Icon name="close" size={13} />
                    </button>
                  </div>
                ))
              ) : (
                <p className="setting-hint">{t.noFolders}</p>
              )}
              <button
                className="add-folder"
                onClick={() => void addFolder()}
                disabled={!native}
                title={!native ? t.browserFolder : ""}
              >
                <Icon name="plus" size={14} />
                {t.addFolder}
              </button>
              {!native && <p className="setting-hint">{t.browserFolder}</p>}
            </fieldset>
            <fieldset disabled={busy}>
              <legend>{t.system}</legend>
              {native && (
                <div className="setting-row">
                  <div>
                    <label>{t.notifications}</label>
                  </div>
                  <button
                    className="quiet-button notification-button"
                    disabled={notificationPermission !== "default"}
                    onClick={() => void allowNotifications()}
                  >
                    {notificationPermission === "granted"
                      ? t.notificationsGranted
                      : notificationPermission === "denied"
                        ? t.notificationsDenied
                        : t.enableNotifications}
                  </button>
                </div>
              )}
              <div className="setting-row">
                <div>
                  <label id="start-label">{t.autostart}</label>
                  {!native && <p>{t.nativeOnly}</p>}
                </div>
                <button
                  className="switch"
                  role="switch"
                  disabled={!native}
                  aria-checked={autoStart}
                  aria-labelledby="start-label"
                  onClick={() => void toggleAutostart()}
                >
                  <span />
                </button>
              </div>
              <label className="select-row">
                <span>{t.language}</span>
                <select
                  aria-label={t.language}
                  value={settings.language}
                  onChange={(e) =>
                    void update({
                      language: e.target.value as Settings["language"],
                    })
                  }
                >
                  <option value="en">English</option>
                  <option value="fr">Français</option>
                </select>
              </label>
            </fieldset>
            {config.coffeeUrl && (
              <button
                className="support-button"
                disabled={!config.coffeeUrl}
                title={!config.coffeeUrl ? t.linkMissing : ""}
                onClick={() => link("coffeeUrl")}
              >
                <Icon name="coffee" />
                {t.support}
                <span>{config.coffeeUrl ? "↗" : t.linkMissing}</span>
              </button>
            )}
            <button className="about-button" onClick={() => setAbout(true)}>
              {t.about}
              <span>
                v{version} <Icon name="chevron" size={12} />
              </span>
            </button>
          </div>
        )}
      </main>
      <footer className="creator-credit">
        <a
          href={config.creatorUrl}
          target="_blank"
          rel="noopener noreferrer"
          onClick={(event) => {
            event.preventDefault();
            link("creatorUrl");
          }}
        >
          Made by @pierrreweb
        </a>
      </footer>
      {notice && (
        <div className="toast" role="status">
          ✓ {notice}
        </div>
      )}
    </div>
  );
  return native ? (
    panel
  ) : (
    <div className="preview-layout">
      <section className="preview-intro">
        <span className="preview-kicker">FONTCATCH / 01</span>
        <h1>{t.previewHeading}</h1>
        <p>{t.previewCaption}</p>
        <div className="preview-mode" role="group" aria-label={t.phase}>
          <button
            className={demo ? "selected" : ""}
            aria-pressed={demo}
            onClick={() => resetDemo(true)}
          >
            {t.demo}
          </button>
          <button
            className={!demo ? "selected" : ""}
            aria-pressed={!demo}
            onClick={() => resetDemo(false)}
          >
            {t.empty}
          </button>
        </div>
      </section>
      <div className="preview-panel-wrap">
        <span className="preview-panel-label">
          {t.phase}
          <span>360 × 480</span>
        </span>
        {panel}
      </div>
    </div>
  );
}
