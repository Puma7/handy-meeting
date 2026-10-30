import { useEffect, useRef, useState } from "react";
import {
  FolderClock,
  RefreshCw,
  Search,
  Pin,
  ArrowRight,
  Trash2,
} from "lucide-react";
import { errorMessage } from "../api";
import { clock } from "../domain";
import type { DesktopApi, Locale, MeetingSummary, SearchPage } from "../types";
/** Renders source-bound search hits and deletion actions without restoring blocked meeting rows. */
export default function HistorySearch({
  api,
  locale,
  onOpen,
  imported = [],
  onErase,
  hiddenMeetingIds,
}: {
  api: DesktopApi;
  onErase?: (meeting: MeetingSummary) => void;
  hiddenMeetingIds?: ReadonlySet<string>;
  locale: Locale;
  onOpen: (id: string, sourceId: string | null) => void;
  /** Imported legacy meetings are read in place and are not part of the search index. */
  imported?: MeetingSummary[];
}) {
  const de = locale === "de";
  const [query, setQuery] = useState(""),
    [offset, setOffset] = useState(0),
    [page, setPage] = useState<SearchPage | null>(null),
    [error, setError] = useState<string | null>(null),
    [busy, setBusy] = useState(false),
    [refresh, setRefresh] = useState(0);
  const generation = useRef(0);
  useEffect(() => {
    const update = () => setRefresh((v) => v + 1);
    window.addEventListener("library-index-updated", update);
    return () => window.removeEventListener("library-index-updated", update);
  }, []);
  useEffect(() => {
    const current = ++generation.current;
    setBusy(true);
    setError(null);
    const timer = setTimeout(
      () => {
        void api
          .searchMeetings?.(query, offset, 25)
          .then((value) => {
            if (generation.current === current) setPage(value);
          })
          .catch((e) => {
            if (generation.current === current) setError(errorMessage(e));
          })
          .finally(() => {
            if (generation.current === current) setBusy(false);
          });
      },
      query ? 250 : 0,
    );
    return () => {
      generation.current++;
      clearTimeout(timer);
    };
  }, [api, query, offset, refresh]);
  return (
    <section className="history-search">
      <div className="section-heading">
        <h2>
          <FolderClock size={18} />
          {de ? "Meetings durchsuchen" : "Search meetings"}
        </h2>
        <button
          className="secondary"
          disabled={busy}
          onClick={() => setRefresh((v) => v + 1)}
        >
          <RefreshCw size={15} />
          {de ? "Aktualisieren" : "Refresh"}
        </button>
      </div>
      <label className="search-field">
        <span>
          <Search size={15} />
          {de
            ? "Suche in Meetingnamen und Transkripten"
            : "Search meeting names and transcripts"}
        </span>
        <input
          type="search"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setOffset(0);
          }}
          placeholder={
            de ? "Begriff, Name oder Thema …" : "Word, name or topic …"
          }
        />
      </label>
      <div className="search-state" role="status">
        {busy
          ? de
            ? "Suchergebnisse werden geladen …"
            : "Loading search results …"
          : page
            ? `${page.total.toLocaleString(locale)} ${de ? "Treffer" : "results"}`
            : ""}
      </div>
      {error && (
        <p role="alert" className="inline-error">
          {error}
        </p>
      )}
      <div className="session-list" aria-busy={busy}>
        {page?.items
          .filter((item) => !hiddenMeetingIds?.has(item.id))
          .map((item, i) => (
            <article
              className="session-row search-result"
              key={item.id + ":" + item.sourceId + ":" + i}
            >
              <div>
                <strong>
                  {item.pinned && <Pin size={14} aria-label="VIP" />}
                  {item.title}
                </strong>
                <p>
                  {new Date(item.startedAt).toLocaleString(locale)} ·{" "}
                  {clock(item.durationMs)}
                </p>
                {item.excerpt && (
                  <p className="search-excerpt">{item.excerpt}</p>
                )}
                {item.sourceId && (
                  <small>
                    {de ? "Originalabschnitt" : "Original section"}:{" "}
                    {clock(item.startMs ?? 0)}–{clock(item.endMs ?? 0)}
                  </small>
                )}
              </div>
              <div className="session-row-actions">
                {onErase && !item.id.startsWith("legacy-") && (
                  <button
                    type="button"
                    className="icon-button erasure-button"
                    aria-label={`${de ? "Meeting dauerhaft löschen" : "Permanently delete meeting"}: ${item.title}`}
                    title={
                      de
                        ? "Meeting dauerhaft löschen"
                        : "Permanently delete meeting"
                    }
                    onClick={() => onErase(item)}
                  >
                    <Trash2 size={16} />
                  </button>
                )}
                <button
                  className="secondary"
                  onClick={() => onOpen(item.id, item.sourceId)}
                >
                  <ArrowRight size={15} />
                  {de ? "Öffnen" : "Open"}
                </button>
              </div>
            </article>
          ))}
        {!busy && page?.total === 0 && (
          <p className="empty">
            {query
              ? de
                ? "Keine Treffer. Prüfe bei Bedarf den Suchindex unter Einstellungen."
                : "No matches. Check the search index in Settings if needed."
              : de
                ? "Der lokale Suchindex wird beim Start aktualisiert. Vorhandene Sitzungen bleiben erhalten."
                : "The local index updates at startup. Existing sessions are preserved."}
          </p>
        )}
      </div>
      {!query && imported.length > 0 && (
        <div className="session-list">
          <h3>
            {de
              ? "Importierte ältere Meetings · nicht durchsuchbar"
              : "Imported earlier meetings · not searchable"}
          </h3>
          {imported.map((item) => (
            <article className="session-row" key={item.id}>
              <div>
                <strong>{item.title}</strong>
                <p>
                  {new Date(item.startedAt).toLocaleString(locale)} ·{" "}
                  {clock(item.durationMs)}
                </p>
              </div>
              <button
                className="secondary"
                onClick={() => onOpen(item.id, null)}
              >
                <ArrowRight size={15} />
                {de ? "Öffnen" : "Open"}
              </button>
            </article>
          ))}
        </div>
      )}
      {page && page.total > 25 && (
        <div className="search-pages">
          <button
            className="secondary"
            disabled={busy || offset === 0}
            onClick={() => setOffset((n) => Math.max(0, n - 25))}
          >
            {de ? "Zurück" : "Previous"}
          </button>
          <span>
            {offset + 1}–{Math.min(offset + 25, page.total)}
          </span>
          <button
            className="secondary"
            disabled={busy || offset + 25 >= page.total}
            onClick={() => setOffset((n) => n + 25)}
          >
            {de ? "Weiter" : "Next"}
          </button>
        </div>
      )}
    </section>
  );
}
