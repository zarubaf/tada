import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { type Api, problemMessage, type ReminderSettings as Settings } from "../api/client";
import { failureOf } from "../api/failure";
import { hasMessage, t } from "../i18n";
import { Link } from "../router/Router";
import { ComboBox } from "../ui/ComboBox";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { SkeletonLines } from "../ui/Skeleton";
import { Switch } from "../ui/Switch";
import styles from "./ReminderSettings.module.css";
import { timeZoneLabel, timeZonesWith } from "./timeZones";

/** The ID of the hint about the Telegram link; focus goes there when the switch becomes disabled. */
const LINK_HINT = "reminders-link-hint";

/** Without a link a member cannot opt in. A member who is in already can always opt out. */
const isBlocked = (settings: Settings) =>
  !settings.telegram_linked && settings.opted_in_at === null;

/** The change that a save makes: the error of a refused change goes to its own control. */
type Change = "opt-in" | "zone";

type State = { kind: "loading" } | { kind: "failed" } | { kind: "loaded"; settings: Settings };

export interface ReminderSettingsProps {
  api: Api;
  /** The page owns the live regions: a result goes to its status region. */
  onStatus: (message: string | undefined) => void;
  /** A failure goes to the alert region of the page. */
  onFailure: (message: string | undefined) => void;
}

/**
 * The reminders of the own account on „Konto“: the opt-in and the time zone. They apply in each
 * organization of the member. An opt-in needs a confirmed Telegram link, because the reminders go
 * to Telegram. Each change has an immediate effect, so there is no submit button.
 */
export function ReminderSettings({ api, onStatus, onFailure }: ReminderSettingsProps) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [busy, setBusy] = useState(false);
  // The text of the time zone input; it follows the saved zone until the member types.
  const [zoneText, setZoneText] = useState("");
  const [zoneError, setZoneError] = useState<string>();
  const title = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => title.current);
  const focusAfterCommit = useFocusAfterCommit();

  const show = useCallback((settings: Settings) => {
    setState({ kind: "loaded", settings });
    setZoneText(timeZoneLabel(settings.time_zone));
  }, []);

  /** Resolves to the settings when they loaded. */
  const read = useCallback(async () => {
    try {
      const { data } = await api.GET("/api/v1/account/reminders");
      if (data) {
        show(data);
        return data;
      }
    } catch {
      // The same failure state as a problem response.
    }
    setState({ kind: "failed" });
    return undefined;
  }, [api, show]);

  /** Resolves to true when the settings loaded. */
  const load = useCallback(async () => (await read()) !== undefined, [read]);

  useEffect(() => {
    void load();
  }, [load]);

  const zones = useMemo(
    () => timeZonesWith(state.kind === "loaded" ? state.settings.time_zone : undefined),
    [state],
  );

  const save = async (change: Change, optIn: boolean, timeZone: string, saved: string) => {
    if (busy) {
      return;
    }
    // A switch that becomes disabled drops its focus: the focus then goes to the hint about the link.
    const switchFocused = document.activeElement?.getAttribute("role") === "switch";
    const keepFocus = (settings: Settings | undefined) => {
      if (switchFocused && settings && isBlocked(settings)) {
        focusAfterCommit(() => document.getElementById(LINK_HINT));
      }
    };
    onFailure(undefined);
    onStatus(undefined);
    setZoneError(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/account/reminders/set", {
        body: { opt_in: optIn, time_zone: timeZone },
      });
      if (result.data) {
        show(result.data);
        keepFocus(result.data);
        onStatus(saved);
      } else if (result.error?.code === "validation-failed") {
        const errors = result.error.errors ?? [];
        const zone = errors.find((error) => error.pointer === "/time_zone")?.code;
        if (zone) {
          setZoneError(
            hasMessage(`reminders-error-${zone}`)
              ? t(`reminders-error-${zone}`)
              : t("problem-validation-failed"),
          );
        }
        if (errors.some((error) => error.code === "telegram-not-linked")) {
          // The link ended: the page shows the truth, with the way to a new link. A member who is
          // in without a link cannot change the zone, because the change repeats the opt-in.
          keepFocus(await read());
          if (change === "zone") {
            setZoneError(t("reminders-error-zone-needs-link"));
          } else {
            onFailure(t("reminders-error-telegram-not-linked"));
          }
        } else if (!zone) {
          onFailure(t("problem-validation-failed"));
        }
      } else {
        onFailure(failureOf(result).message);
      }
    } catch {
      onFailure(problemMessage(undefined));
    }
    setBusy(false);
  };

  return (
    <section className={styles.section} aria-labelledby="reminders-title">
      <h2 id="reminders-title" ref={title} tabIndex={-1} className={styles.title}>
        {t("reminders-title")}
      </h2>
      <p className={styles.intro}>{t("reminders-intro")}</p>
      {state.kind === "loading" && <SkeletonLines label={t("reminders-loading")} />}
      {state.kind === "failed" && (
        <InlineError
          message={problemMessage(undefined)}
          onRetry={() =>
            retry(() => {
              setState({ kind: "loading" });
              return load();
            })
          }
          announce={retried ? "focus" : "alert"}
        />
      )}
      {state.kind === "loaded" && (
        <Controls
          settings={state.settings}
          busy={busy}
          zones={zones}
          zoneText={zoneText}
          onZoneText={(text) => {
            setZoneText(text);
            setZoneError(undefined);
          }}
          zoneError={zoneError}
          onOptIn={(optIn) =>
            void save(
              "opt-in",
              optIn,
              state.settings.time_zone,
              t(optIn ? "reminders-on" : "reminders-off"),
            )
          }
          onZone={(zone) =>
            void save(
              "zone",
              state.settings.opted_in_at !== null,
              zone,
              t("reminders-time-zone-saved", { zone: timeZoneLabel(zone) }),
            )
          }
        />
      )}
    </section>
  );
}

function Controls({
  settings,
  busy,
  zones,
  zoneText,
  onZoneText,
  zoneError,
  onOptIn,
  onZone,
}: {
  settings: Settings;
  busy: boolean;
  zones: string[];
  zoneText: string;
  onZoneText: (text: string) => void;
  zoneError: string | undefined;
  onOptIn: (optIn: boolean) => void;
  onZone: (zone: string) => void;
}) {
  const optedIn = settings.opted_in_at !== null;
  const blocked = isBlocked(settings);
  const linked = settings.telegram_linked;
  const savedLabel = timeZoneLabel(settings.time_zone);
  // While the input shows the saved zone, the list shows all zones; typed text filters it.
  const query = zoneText === savedLabel ? "" : zoneText.trim().toLowerCase();
  const options = zones
    .map((zone) => ({ id: zone, label: timeZoneLabel(zone) }))
    .filter((option) => option.label.toLowerCase().includes(query));

  return (
    <div className={styles.controls}>
      <div className={styles.optIn}>
        <Switch
          label={t("reminders-opt-in")}
          isSelected={optedIn}
          onChange={onOptIn}
          isDisabled={blocked}
          isPending={busy}
          {...(linked ? {} : { "aria-describedby": LINK_HINT })}
        />
        {!linked && (
          <p id={LINK_HINT} tabIndex={-1} className={styles.help}>
            {t(optedIn ? "reminders-link-lost" : "reminders-not-linked")}{" "}
            <Link to="/settings/telegram" className={styles.link}>
              {t("reminders-link-telegram")}
            </Link>
          </p>
        )}
      </div>
      <ComboBox
        label={t("reminders-time-zone")}
        help={t("reminders-time-zone-help")}
        options={options}
        inputValue={zoneText}
        onInputChange={onZoneText}
        selectedKey={settings.time_zone}
        onSelectionChange={(zone) => {
          if (zone && zone !== settings.time_zone && !busy) {
            onZone(zone);
          }
        }}
        error={zoneError}
      />
    </div>
  );
}
