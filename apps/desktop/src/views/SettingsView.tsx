import { FolderOpen, RefreshCw } from "lucide-react";
import type { AppState, DesktopApi, Locale, Theme } from "../types";
import type { MessageKey } from "../i18n";
import RetentionPanel from "./RetentionPanel";
import UpdatePanel from "./UpdatePanel";

/**
 * Presents read-only program paths and delegates native selection/import to the parent.
 * Displayed path text does not grant execution permission or start capture.
 */
export default function SettingsView({
  api,
  app,
  locale,
  theme,
  t,
  busy,
  blocked,
  connected,
  refreshingHandy,
  preferencesPersistable,
  handyPath,
  onChooseHandy,
  onRefreshHandy,
  onRefreshDevices,
  onImportLegacy,
  onRebuildIndex,
  onLocale,
  onTheme,
}: {
  api: DesktopApi;
  app: AppState | null;
  locale: Locale;
  theme: Theme;
  t: Record<MessageKey, string>;
  busy: boolean;
  blocked: boolean;
  connected: boolean;
  refreshingHandy: boolean;
  preferencesPersistable: boolean;
  handyPath: string;
  onChooseHandy: () => void;
  onRefreshHandy: () => void;
  onRefreshDevices: () => void;
  onImportLegacy: () => void;
  onRebuildIndex: () => void;
  onLocale: (locale: Locale) => void;
  onTheme: (theme: Theme) => void;
}) {
  const de = locale === "de";
  const unavailable = busy || blocked || !connected;
  return (
    <>
      <div className="settings-sections">
        {!preferencesPersistable && (
          <div className="notice warning">{t.preferencesError}</div>
        )}
        <section>
          <h2>{t.installation}</h2>
          <p>{t.executableHelp}</p>
          <label>
            {t.executable}
            <input
              value={handyPath || app?.handy?.executable || ""}
              readOnly
              placeholder={de ? "Automatisch erkennen" : "Detect automatically"}
            />
          </label>
          <div className="settings-actions">
            <button
              className="secondary"
              disabled={
                unavailable || refreshingHandy || !api.chooseHandyExecutable
              }
              onClick={onChooseHandy}
            >
              <FolderOpen size={15} />
              {de ? "Handy-Programm auswählen …" : "Choose Handy program …"}
            </button>
            <button
              className="secondary"
              disabled={unavailable || refreshingHandy}
              onClick={onRefreshHandy}
            >
              <RefreshCw size={15} />
              {t.refresh}
            </button>
          </div>
          <p>
            {t.version}:{" "}
            {app?.handy?.version ??
              (app?.handy?.executable ? t.handyVersionMissing : t.notFound)}
          </p>
          <p className="muted">
            {de
              ? "Eine andere Programmdatei wird im Dateidialog des Betriebssystems gewählt und geprüft. Manuelle Freigaben gelten für diesen App-Start; ein eingetippter Pfad erteilt keine Freigabe."
              : "Choose another program in the operating system's file dialog. Manual approvals last for this app session; a typed path does not grant permission to start it."}
          </p>
        </section>
        <section>
          <h2>{t.theme}</h2>
          <div className="settings-grid">
            <label>
              {t.language}
              <select
                value={locale}
                onChange={(e) => onLocale(e.target.value as Locale)}
              >
                <option value="de">Deutsch</option>
                <option value="en">English</option>
              </select>
            </label>
            <label>
              {t.theme}
              <select
                value={theme}
                onChange={(e) => onTheme(e.target.value as Theme)}
              >
                <option value="system">{t.systemTheme}</option>
                <option value="light">{t.light}</option>
                <option value="dark">{t.dark}</option>
              </select>
            </label>
          </div>
        </section>
        <section>
          <h2>{t.system}</h2>
          <button
            className="secondary"
            disabled={unavailable}
            onClick={onRefreshDevices}
          >
            <RefreshCw size={15} />
            {t.refreshDevices}
          </button>
        </section>
        <section>
          <h2>{t.storage}</h2>
          <p>{t.legacyHelp}</p>
          {app?.dataDirectory && (
            <code className="storage-path">{app.dataDirectory}</code>
          )}
        </section>
      </div>
      <UpdatePanel
        api={api}
        locale={locale}
        blocked={blocked || Boolean(app?.notesRunning)}
      />
      {api.rebuildSearchIndex && (
        <section className="legacy-panel">
          <h2>{de ? "Lokaler Suchindex" : "Local search index"}</h2>
          <p className="muted">
            {de
              ? "Der Suchindex beschleunigt große Historien. Er wird lokal aus deinen Sitzungen aufgebaut; Originale werden nicht geändert."
              : "The local index speeds up large histories. Rebuilding never changes original sessions."}
          </p>
          <button
            className="secondary"
            disabled={unavailable}
            onClick={onRebuildIndex}
          >
            <RefreshCw size={15} />
            {de ? "Suchindex aktualisieren" : "Update search index"}
          </button>
        </section>
      )}
      <RetentionPanel
        api={api}
        locale={locale}
        models={app?.handy?.models ?? []}
      />
      <section className="legacy-panel">
        <h2>{t.legacy}</h2>
        <p className="muted">{t.legacyHelp}</p>
        <button
          className="secondary"
          disabled={unavailable || !api.chooseLegacyManifest}
          onClick={onImportLegacy}
        >
          <FolderOpen size={15} />
          {de
            ? "Sitzungsdatei auswählen und nur lesbar öffnen …"
            : "Choose session file and open read-only …"}
        </button>
        <p className="muted">
          {de
            ? "Wähle die vorhandene session.json im Dateidialog. Abbrechen verändert nichts; die Originaldateien bleiben erhalten."
            : "Choose the existing session.json in the file dialog. Cancelling changes nothing; original files are preserved."}
        </p>
      </section>
    </>
  );
}
