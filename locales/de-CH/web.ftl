## Web client, German (Switzerland). See ADR 0005.
## The keys are English and stable. A new key needs the same key in each locale.

app-name = tada

## Events

events-title = Anlässe
events-column-key = Kürzel
events-column-name = Name
events-column-time-zone = Zeitzone
events-column-created = Erfasst am
events-empty-title = Noch keine Anlässe erfasst
events-empty-text = Hier erscheinen die Anlässe Ihrer Organisation.
events-load-more = Weitere Anlässe laden
events-loading = Anlässe werden geladen
events-loaded-more = Weitere Anlässe geladen.

## Errors

retry = Erneut versuchen
# $requestId: the ID that an operator needs to find the log lines (ADR 0035).
problem-request-id = Fehler-ID: { $requestId }
problem-unauthenticated = Sie sind nicht angemeldet.
problem-organization-required = Wählen Sie eine Organisation.
problem-recent-sign-in-required = Melden Sie sich für diesen Schritt neu an. Aus Sicherheitsgründen geht er nur in den ersten 15 Minuten nach der Anmeldung.
sign-in-again = Neu anmelden
problem-forbidden = Sie haben keine Berechtigung für diese Aktion.
# The no-access state of a settings area for a member without the role (doc/design/components.md).
no-access-title = Sie haben keinen Zugriff auf diesen Bereich.
no-access-text = Die Organisationsleitung oder die Administration kann Ihnen die Rolle dafür geben.
problem-unavailable = Der Dienst ist im Moment nicht erreichbar. Versuchen Sie es in einigen Minuten erneut.
problem-general-4xx = Die Anfrage ist ungültig.
problem-general-5xx = Ein unerwarteter Fehler ist aufgetreten.
problem-network = Keine Verbindung zum Server. Prüfen Sie Ihre Internetverbindung.
chunk-failed = Die Seite konnte nicht geladen werden. Vielleicht wurde die App aktualisiert.
chunk-failed-reload = Seite neu laden

## Fields of the core catalog (ADR 0049). The ID is field-<key>, and field-<key>-<choice> for a choice.

field-date_window = Zeitfenster
field-exact_dates = Genaue Daten
field-duration_days = Dauer in Tagen
field-audience = Publikum
field-audience-public = Öffentlich
field-audience-members = Mitglieder
field-audience-invited = Geladene Gäste
field-visitor_estimate = Erwartete Besucher pro Tag
field-entry_fee_policy = Eintrittspolitik
field-entry_fee_policy-free = Gratis
field-entry_fee_policy-low = Tiefer Eintritt
field-entry_fee_policy-regular = Normaler Eintritt
field-entry_fee_adult = Eintritt Erwachsene
field-components = Programmteile
field-components-airshow = Flugshow
field-components-static_display = Flugzeugausstellung
field-components-catering = Verpflegung
field-components-passenger_flights = Passagierflüge
field-components-exhibition = Ausstellung
field-venue = Ort

## Units, and values as the web client shows them (ADR 0049).
unit-day =
    { $count ->
        [one] Tag
       *[other] Tage
    }
unit-person_per_day = Personen pro Tag
value-yes = Ja
value-no = Nein
value-approximate = ca. { $value }
value-unsupported = Nicht darstellbar
value-reference-document = Dokument
value-reference-event = Anlass

## The sheet and the evidence panel.
sheet-close = Schliessen
side-panel-close = Schliessen
side-panel-previous = Vorheriger Eintrag
side-panel-next = Nächster Eintrag
# $index: the place of the record in the list, from 1. $total: the number of records in the list.
side-panel-position = { $index } von { $total }
evidence-accepted-by = Angenommen von
evidence-accepted-at = Angenommen am
evidence-sources = Belege
evidence-none = Für diesen Wert gibt es keinen Beleg.
evidence-captured = erfasst am { $time }
evidence-page = Seite { $page }
evidence-source-version = Quellversion { $id }
evidence-document-version = { $name }, Version { $number }
evidence-author-unknown = Unbekanntes Mitglied
evidence-author-ai = KI-Client
evidence-author-service = Dienst
evidence-author-other = Unbekannter Absender

## The state of knowledge of a value (doc/design/tokens.md).
knowledge-accepted = Bestätigt
knowledge-proposed = Vorschlag
knowledge-assumption = Annahme
knowledge-unknown = Unbekannt
knowledge-conflict = Konflikt

## The kinds and display pieces of work records (ADR 0082).
record-kind-action = Aufgabe
record-kind-commitment = Zusage
record-kind-milestone = Meilenstein
record-kind-decision = Entscheid
record-kind-risk = Risiko
risk-status-open = offen
risk-status-mitigated = gemindert
risk-status-occurred = eingetreten
risk-status-closed = geschlossen
risk-level-low = niedrig
risk-level-medium = mittel
risk-level-high = hoch
overdue = überfällig
chip-workstream = Bereich

## Navigation

not-found-title = Seite nicht gefunden
not-found-text = Diese Adresse gibt es nicht. Prüfen Sie den Link oder wählen Sie einen Eintrag in der Navigation.

## Session

session-loading = Sitzung wird geladen
page-loading = Seite wird geladen
skip-link = Zum Inhalt springen
shell-nav = Hauptnavigation
# The labelled region around the content of the sidebar: the brand, the switchers and the links.
shell-sidebar = Seitenleiste
# The toggle of the sidebar in the top bar. On narrow layouts it opens the drawer.
shell-sidebar-close = Seitenleiste ausblenden
shell-sidebar-open = Seitenleiste einblenden
shell-drawer-open = Navigation öffnen
shell-drawer-title = Navigation
shell-drawer-close = Navigation schliessen
# The bottom bar on narrow layouts: the fourth item opens a menu with the other destinations.
nav-more = Mehr
nav-more-menu = Weitere Bereiche
nav-my-work = Meine Arbeit
nav-work = Arbeit
nav-timeline = Zeitplan
nav-events = Anlässe
nav-persons = Personen
nav-institutions = Institutionen
nav-inbox = Eingang
# $count: the number of changesets that wait for a review.
nav-inbox-count = { $count } offen
organization-switcher = Organisation
member-menu = Mitgliedermenü
# $name: the name of the member. The button of the member menu in the top bar.
member-menu-trigger = Konto von { $name }
member-menu-versions = Versionen ({ $build })
sign-out = Abmelden
nav-privacy = Datenschutz
# The command box in the top bar and the command palette it opens.
command-box = Suchen oder Befehl ausführen…
command-box-label = Suchen oder Befehl ausführen
command-palette-label = Befehle
command-palette-search = Befehl oder Ziel eingeben
command-group-goto = Gehe zu
command-group-events = Anlässe
command-group-actions = Aktionen
command-new-action = Neue Aufgabe
command-new-event = Neuer Anlass
# $count: the number of results that match the typed text.
command-results = { $count ->
    [one] 1 Treffer
   *[other] { $count } Treffer
}
# The three choices of the color theme.
theme-system = System
theme-light = Hell
theme-dark = Dunkel
# $name: the current choice, as the text of one of the three messages above.
theme-toggle = Farbschema: { $name }
# The announcement for screen readers after a change of the choice. $name as above.
theme-announcement = Farbschema geändert: { $name }

# The event switcher at the head of the sidebar.
event-switcher-pick = Anlass wählen
# $name: the name of the current event.
event-switcher-label = Anlass wechseln: { $name }
event-switcher-menu = Anlässe
# $name: the name of the current event, in the list of the events.
event-switcher-current-item = { $name }, aktuell
event-switcher-loading = Anlässe werden geladen …
event-switcher-failed = Die Anlässe liessen sich nicht laden.
event-switcher-all = Alle Anlässe
event-switcher-create = Neuer Anlass
event-switcher-closed = abgeschlossen

# The versions of tada (ADR 0075): the footer, the list of the components and the prompt to reload.
version-outdated-title = Eine neue Version von tada ist installiert.
version-outdated-text = Diese Seite läuft noch mit der alten Version. Laden Sie die Seite neu, damit alles richtig funktioniert.
version-reload = Seite neu laden
versions-title = Versionen
versions-loading = Versionen werden geladen
versions-components = Bestandteile
versions-component = Bestandteil
versions-version = Version
versions-web = Web-Client (diese Seite)
versions-server = Server
versions-schema = Datenbankschema
versions-api = API
versions-mcp = MCP-Server
versions-roles = Laufende Prozesse
versions-roles-help = Jeder Prozess meldet sich regelmässig. Die Zeit zeigt seine letzte Meldung.
versions-role = Prozess
versions-last-seen = Letzte Meldung
versions-differs = Andere Version
versions-unknown = unbekannt
versions-no-roles = Kein Prozess hat sich gemeldet.

## Sign-in, magic link and invitation (ADR 0008, ADR 0056)

sign-in-title = Anmelden
sign-in-text = Wir senden Ihnen einen Link per E-Mail. Mit dem Link melden Sie sich an.
sign-in-email = E-Mail-Adresse (Pflichtfeld)
sign-in-submit = Anmeldelink senden
sign-in-error-email = Das ist keine gültige E-Mail-Adresse. Geben Sie eine Adresse wie name@example.org ein.
sign-in-sent = Wenn die Adresse bekannt ist, ist eine E-Mail mit einem Anmeldelink unterwegs oder schon in Ihrem Postfach. Verwenden Sie den Link in der neuesten E-Mail.
problem-rate-limited = Zu viele Anfragen. Versuchen Sie es in einigen Minuten erneut.
# $seconds: the wait that the server asks for (Retry-After).
problem-rate-limited-seconds =
    Zu viele Anfragen. Versuchen Sie es in { $seconds } { $seconds ->
        [one] Sekunde
       *[other] Sekunden
    } erneut.
# $minutes: the wait that the server asks for, rounded up.
problem-rate-limited-minutes =
    Zu viele Anfragen. Versuchen Sie es in { $minutes } { $minutes ->
        [one] Minute
       *[other] Minuten
    } erneut.
magic-link-title = Anmelden
magic-link-text = Mit dem Klick melden Sie sich in tada an.
# $email: the masked address of the account, for example „a…@example.org“.
magic-link-account = Konto: { $email }. Ist das nicht Ihre E-Mail-Adresse, melden Sie sich nicht an.
magic-link-submit = Anmelden
magic-link-invalid = Dieser Link ist ungültig oder abgelaufen.
to-sign-in = Zur Anmeldung
invitation-title = Einladung
# $organization: the name of the organization. $role: the role, for example „Mitglied“.
invitation-text = Sie sind eingeladen: Organisation { $organization }, Rolle { $role }.
invitation-accept = Einladung annehmen
invitation-invalid = Diese Einladung ist ungültig oder abgelaufen.
invitation-accepted-sign-in = Die Einladung ist angenommen. Sie sind schon Mitglied einer anderen Organisation in tada. Melden Sie sich jetzt mit Ihrer E-Mail-Adresse an.
invitation-loading = Einladung wird geladen
invitation-privacy-title = Datenschutz
role-owner = Organisationsleitung
role-admin = Administration
role-member = Mitglied

## Choice of the organization (ADR 0056)

choose-organization-title = Organisation wählen
choose-organization-text = Wählen Sie die Organisation, mit der Sie arbeiten.
choose-organization-empty-title = Keine Organisation
choose-organization-empty-text = Sie sind noch in keiner Organisation Mitglied. Bitten Sie eine Administration um eine Einladung.

## Event screens (create, event page and event memberships)

events-create = Anlass erfassen
event-create-title = Anlass erfassen
event-create-submit = Anlass erfassen
event-create-key-label = Kürzel (Pflichtfeld)
event-create-name-label = Name (Pflichtfeld)
event-create-time-zone-label = Zeitzone (Pflichtfeld)
event-create-key-help = Zwei bis acht Grossbuchstaben oder Ziffern, zum Beispiel FLY28.
event-create-time-zone-help = Eine IANA-Zeitzone, zum Beispiel Europe/Zurich.
event-error-key = Das Kürzel hat 2 bis 8 Grossbuchstaben oder Ziffern.
event-error-key-taken = Dieses Kürzel ist schon vergeben.
event-error-name = Der Name hat 1 bis 200 Zeichen.
event-error-time_zone = Diese Zeitzone ist unbekannt.
event-nav = Anlass
event-nav-overview = Übersicht
event-nav-members = Mitglieder
event-nav-documents = Dokumente
event-nav-actions = Aufgaben
event-nav-timeline = Zeitplan
event-nav-registers = Register
event-nav-workstreams = Arbeitsbereiche
register-nav = Register
register-nav-commitments = Zusagen
register-nav-milestones = Meilensteine
register-nav-decisions = Entscheide
event-overview-loading = Übersicht wird geladen
event-overview-questions = Offene Fragen
event-overview-questions-none = Es gibt keine offenen Fragen.
event-overview-facts = Fakten
event-overview-evidence = Beleg
event-overview-evidence-of = Beleg zu { $label }
event-overview-proposals = Vorschläge
event-overview-proposals-none = Es gibt keine offenen Vorschläge.
event-overview-proposals-note = Diese Werte sind noch nicht geprüft. Sie ersetzen keinen bestätigten Wert.
event-overview-proposed-at = vorgeschlagen am { $time }
event-loading = Anlass wird geladen
event-close = Anlass abschliessen
event-close-title = Anlass abschliessen?
event-close-text = Ein abgeschlossener Anlass ist schreibgeschützt. Niemand ändert ihn mehr, bis die Organisationsleitung ihn wieder öffnet.
event-close-cancel = Abbrechen
event-closed-on = Abgeschlossen am { $date }
event-closed = Abgeschlossen
event-closed-note = Dieser Anlass ist schreibgeschützt.
event-closed-redacted = Die Aufbewahrungsfrist ist abgelaufen. Der Inhalt wurde entfernt. Der Anlass lässt sich nicht mehr öffnen.
event-reopen = Wieder öffnen
event-reopen-title = Anlass wieder öffnen?
event-reopen-text = Der Anlass lässt sich danach wieder ändern. Schliessen Sie ihn später erneut ab.
problem-not-found = Das gibt es nicht, oder Sie dürfen es nicht sehen.
problem-validation-failed = Die Eingabe ist ungültig.
problem-record-version-conflict = Jemand hat diesen Eintrag inzwischen geändert. Versuchen Sie es erneut.
problem-invalid-transition = Diese Änderung ist im aktuellen Zustand nicht möglich.
problem-event-closed = Dieser Anlass ist abgeschlossen. Er lässt sich nicht mehr ändern. Die Organisationsleitung kann ihn wieder öffnen.
problem-payload-too-large = Die Datei ist zu gross.
problem-unsupported-media-type = Dieser Dateityp ist nicht erlaubt.

## Documents

# The `validation-failed` entry `quota-exceeded` on `file`: the upload is over the storage quota of the organization (ADR 0043).
document-error-file-quota-exceeded = Der Speicherplatz der Organisation reicht für diese Datei nicht aus.
documents-title = Dokumente
documents-loading = Dokumente werden geladen
documents-search = Dokumente suchen
documents-search-submit = Suchen
documents-search-reset = Filter zurücksetzen
documents-none-title = Noch keine Dokumente
documents-none-text = Laden Sie die erste Datei hoch, damit das Team sie findet.
documents-no-match = Keine Treffer für diese Filter.
documents-no-match-text = Ändern Sie den Suchbegriff oder setzen Sie den Filter zurück.
documents-more = Mehr laden
documents-upload = Datei hochladen
documents-uploading = Datei wird hochgeladen
documents-uploaded = Hochgeladen: { $name }
documents-upload-limits = Der Server prüft den Dateityp und die Grösse jeder Datei.
documents-column-id = Nr.
documents-column-name = Name
documents-column-type = Typ
documents-type-pdf = PDF
documents-type-text = Text
documents-type-office = Office-Dokument
documents-type-image = Bild
documents-type-other = Datei
documents-column-size = Grösse
documents-column-version = Version
documents-column-created = Hochgeladen am
documents-no-scan = Dateien werden nicht auf Schadsoftware geprüft.
document-loading = Dokument wird geladen
document-back = Zurück zu den Dokumenten
document-versions-title = Versionen
document-column-number = Version
document-column-file = Datei
document-column-hash = Prüfsumme
document-column-uploader = Hochgeladen von
document-column-created = Zeitpunkt
document-column-actions = Aktionen
document-uploader-unknown = Unbekannt
document-download = Herunterladen
document-download-of = { $name } herunterladen, Version { $number }
document-draft = Entwurf
document-draft-status = Entwurf ({ $status })
document-status-draft = in Arbeit
document-status-review = in Prüfung
document-status-approved = freigegeben
document-status-superseded = ersetzt
document-status-archived = archiviert
document-draft-no-download = Ein Entwurf hat keine Datei zum Herunterladen.
document-preview-title = Vorschau der neuesten Version
document-preview-of = Vorschau von { $name }
document-preview-open = Vorschau in neuem Tab öffnen
document-preview-none = Für diesen Dateityp gibt es keine Vorschau. Laden Sie die Datei herunter.

## Drafts: approval, changed facts and the difference of two versions (ADR 0051)

document-facts-changed-title = Fakten geändert
document-facts-changed-text = Mindestens ein Fakt, den die neueste Version nennt, hat seither eine neue Version. Die Version bleibt unverändert. Ein neuer Entwurf kann die neuen Werte nennen.
document-approve = Version freigeben
# $number: the number of the approved version.
document-approved = Version { $number } ist freigegeben.
document-approve-invalid = Diese Version kann nicht mehr freigegeben werden. Die Seite zeigt den aktuellen Stand.
document-diff-link = Unterschiede
# $number: the number of the older version.
document-diff-link-to = Unterschiede zu Version { $number }
document-diff-back = Zurück zum Dokument
# $from: the number of the older version. $to: the number of the newer version.
document-diff-title = Unterschiede: Version { $from } zu Version { $to }
document-diff-title-plain = Unterschiede
document-diff-missing = Wählen Sie zwei Entwurfsversionen in der Liste der Versionen aus.
document-diff-facts = Fakten
document-diff-facts-none = Die Entwürfe nennen dieselben Fakten.
document-diff-fact-unknown = Unbekannter Fakt
# $label: the label of the field. $from, $to: fact versions.
document-diff-fact-changed = { $label }: Version { $from } zu Version { $to }
# $label: the label of the field. $version: the fact version.
document-diff-fact-added = Neu: { $label }, Version { $version }
document-diff-fact-removed = Entfallen: { $label }, Version { $version }
document-diff-lines = Zeilen
document-diff-lines-same = Die Zeilen sind gleich.
document-diff-column-change = Änderung
document-diff-column-old = Alt
document-diff-column-new = Neu
document-diff-column-text = Text
document-diff-added = hinzugefügt
document-diff-removed = entfernt
document-diff-unchanged = unverändert

## Drafts: the rendered text, its sources and the lint (ADR 0051, ADR 0058)

# The tombstone of a redacted value (ADR 0072).
redacted = entfernt
# $value: the value as the member sees it, with its state of knowledge. $label: the field.
draft-fact-evidence = { $value }, Beleg zu { $label }
# $version: the fact version that the draft cites.
draft-fact-older = (Version { $version } des Fakts, seither geändert)
# $line: the line of the draft, from 1. $kind: what the lint found.
draft-lint-line = Zeile { $line }: { $kind }
draft-lint-number = Zahl ausserhalb eines Fakt-Links
draft-lint-date = Datum ausserhalb eines Fakt-Links
draft-lint-money = Betrag ausserhalb eines Fakt-Links
draft-lint-raw-html = HTML im Text. tada zeigt es nicht an.
draft-lint-other = Anderer Hinweis
draft-sources = Quellen
# $number: the number of the source in the text.
draft-source-number = Quelle { $number }
# $id: the start of the ID of the source version.
draft-source-version = Quellversion { $id }

## Event memberships

event-members-title = Mitglieder des Anlasses
event-members-column-name = Name
event-members-column-role = Rolle
event-members-column-actions = Aktionen
event-members-loading = Mitglieder werden geladen
# $name: the display name of the member.
event-members-role-of = Rolle von { $name }
event-members-remove = Entfernen
event-members-remove-of = { $name } entfernen
event-members-add-title = Rolle vergeben
event-members-add-member = Mitglied
event-members-add-placeholder = Mitglied wählen
event-members-add-submit = Mitglied hinzufügen
# $name: the display name of the member.
event-members-added = { $name } hat jetzt eine Rolle in diesem Anlass.
event-members-add-none = Alle Mitglieder der Organisation haben schon eine Rolle in diesem Anlass.
role-event-manager = Anlassleitung
role-event-contributor = Mitarbeit
role-event-viewer = Lesezugriff
# The refusal to remove or demote the last event manager (`invalid-transition` on this page).
event-members-last-manager = Ein Anlass braucht mindestens eine Anlassleitung. Geben Sie zuerst jemand anderem diese Rolle.
event-members-conflict = Jemand hat diese Mitglieder inzwischen geändert. Die Liste ist neu geladen. Versuchen Sie es erneut.
# $name: the display name of the member.
event-members-remove-title = { $name } entfernen?
event-members-remove-text = { $name } verliert die Rolle in diesem Anlass und sieht den Anlass nicht mehr.
event-members-remove-self = Sie entfernen Ihre eigene Rolle als Anlassleitung. Danach können Sie die Mitglieder dieses Anlasses womöglich nicht mehr verwalten.
event-members-remove-cancel = Abbrechen
members-end-sessions = Sitzungen beenden
# $name: the display name of the member.
members-end-sessions-of = Sitzungen von { $name } beenden
members-end-sessions-title = Sitzungen beenden?
# $name: the display name of the member.
members-end-sessions-text = tada meldet { $name } auf allen Geräten ab, zum Beispiel nach dem Verlust eines Geräts. { $name } bleibt Mitglied und meldet sich mit einem neuen Link an.
# $name: the display name of the member.
members-end-sessions-ended = Sitzungen von { $name } beendet.

## Members and invitations (settings)

nav-settings = Einstellungen
members-title = Mitglieder
members-column-name = Name
members-column-email = E-Mail
members-column-role = Rolle
members-column-actions = Aktionen
members-column-invited = Eingeladen am
members-loading = Mitglieder werden geladen
members-load-more = Weitere Mitglieder laden
members-remove = Entfernen
# $name: the display name of the member.
members-remove-of = { $name } entfernen
members-remove-title = Mitglied entfernen?
# $name: the display name of the member.
members-remove-text = { $name } verliert den Zugang zur Organisation und zu allen Anlässen. tada meldet { $name } auf allen Geräten ab.
members-remove-cancel = Abbrechen
invitations-title = Offene Einladungen
invitations-loading = Einladungen werden geladen
invitations-empty-title = Keine offenen Einladungen
invitations-empty-text = Neue Einladungen erscheinen hier, bis die Person sie annimmt.
invitations-revoke = Widerrufen
# $name: the display name of the invited person.
invitations-revoke-of = Einladung an { $name } widerrufen
invitations-revoke-title = Einladung widerrufen?
# $name: the display name of the invited person.
invitations-revoke-text = Der Link in der Einladung an { $name } funktioniert danach nicht mehr.
invite-title = Mitglied einladen
invite-email = E-Mail-Adresse (Pflichtfeld)
invite-name = Name (Pflichtfeld)
invite-role = Rolle
invite-submit = Einladen
# $name: the display name of the invited person.
invite-sent = Einladung an { $name } gesendet.
invite-error-email = Das ist keine gültige E-Mail-Adresse.
invite-error-display_name = Der Name hat 1 bis 100 Zeichen.
members-leave = Organisation verlassen
members-leave-title = Organisation verlassen?
members-leave-text = Sie verlieren den Zugang zur Organisation und zu allen Anlässen.
members-leave-warning = Sie entfernen sich selbst. Danach sehen Sie diese Organisation nicht mehr und können nur über eine neue Einladung zurückkehren. tada meldet Sie auf allen Geräten ab.
members-remove-last = Die Organisation braucht mindestens eine Organisationsleitung, und jeder Anlass braucht eine Anlassleitung. Bestimmen Sie zuerst jemand anderen.
members-conflict = Jemand hat die Mitglieder inzwischen geändert. Die Liste ist neu geladen. Versuchen Sie es erneut.
members-loaded-more = Weitere Mitglieder geladen.

## Settings navigation and Telegram link (settings)

settings-nav = Einstellungen
settings-nav-members = Mitglieder
settings-nav-telegram = Telegram
settings-nav-tokens = API-Token
settings-nav-organization = Organisation
settings-nav-ai-pm = AI-PM
settings-nav-account = Konto
settings-loading = Einstellungen werden geladen
account-title = Konto
account-sessions-title = Sitzungen
account-sessions-text = Haben Sie ein Gerät verloren, oder meldet sich jemand anderes mit Ihrem Konto an? Melden Sie sich überall ab. Danach melden Sie sich mit einem neuen Link an.
account-sign-out-everywhere = Überall abmelden
account-sign-out-everywhere-title = Überall abmelden?
account-sign-out-everywhere-text = tada beendet alle Ihre Sitzungen, in jeder Organisation und auf jedem Gerät, auch diese hier. Ihre Mitgliedschaften bleiben.

# Reminders of the own account (ADR 0076): the opt-in and the time zone.
reminders-title = Erinnerungen
reminders-intro = tada schickt Ihnen in Telegram eine Übersicht der Aufgaben, für die Sie verantwortlich sind. Die Einstellung gilt in jeder Ihrer Organisationen.
reminders-loading = Erinnerungen werden geladen
reminders-opt-in = Erinnerungen in Telegram erhalten
reminders-not-linked = Für Erinnerungen braucht tada ein verknüpftes Telegram-Konto.
reminders-link-telegram = Telegram verknüpfen
reminders-link-lost = Ihr Telegram-Konto ist nicht mehr verknüpft. tada schickt keine Erinnerungen, bis Sie es wieder verknüpfen.
reminders-time-zone = Zeitzone
reminders-time-zone-help = Die Ruhezeit Ihrer Organisation gilt in dieser Zeitzone. In der Ruhezeit schickt tada keine Erinnerungen.
reminders-on = Erinnerungen sind eingeschaltet.
reminders-off = Erinnerungen sind ausgeschaltet.
reminders-time-zone-saved = Zeitzone { $zone } gespeichert.
reminders-error-telegram-not-linked = Ihr Telegram-Konto ist nicht mehr verknüpft. Verknüpfen Sie es zuerst, dann schalten Sie die Erinnerungen ein.
reminders-error-invalid-time-zone = tada kennt diese Zeitzone nicht. Wählen Sie eine Zeitzone aus der Liste.
reminders-error-zone-needs-link = Ihr Telegram-Konto ist nicht mehr verknüpft. Verknüpfen Sie es wieder, oder schalten Sie die Erinnerungen aus. Dann können Sie die Zeitzone ändern.
telegram-title = Telegram verknüpfen
telegram-intro = Verknüpfen Sie Ihr Telegram-Konto mit tada, um tada im Chat zu nutzen.
telegram-steps-title = So geht es
telegram-step-create = Erstellen Sie hier einen Code.
telegram-step-send = Senden Sie den Code dem Bot in Telegram. Der Bot nennt Ihr tada-Konto. Bestätigen Sie dort mit /bestaetigen.
telegram-step-confirm = Bestätigen Sie die Anfrage hier in tada.
telegram-check = Bestätigen Sie nur, wenn der Name Ihr eigenes Telegram-Konto zeigt. Eine Anfrage, die Sie nicht kennen, ignorieren Sie. Ohne Ihre Bestätigung verknüpft tada nichts.
telegram-code-create = Code erstellen
telegram-code-label = Ihr Code
# $time: the time of the expiry.
telegram-code-expires = Der Code läuft ab um { $time } Uhr und gilt nur einmal. Sie sehen ihn nur jetzt.
telegram-requests-title = Offene Anfragen
telegram-requests-loading = Anfragen werden geladen
telegram-requests-refresh = Aktualisieren
telegram-requests-empty-title = Keine offenen Anfragen
telegram-requests-empty-text = Sobald Sie den Code im Bot mit /bestaetigen annehmen, erscheint hier das Telegram-Konto.
telegram-column-name = Telegram-Name
telegram-column-id = Telegram-ID
telegram-column-claimed = Angefragt um
telegram-column-actions = Aktionen
telegram-confirm = Bestätigen
# $name: the Telegram name of the account.
telegram-confirm-of = { $name } bestätigen
telegram-confirm-title = Telegram-Konto verknüpfen?
# $name: the Telegram name of the account. $id: its Telegram ID. $time: the time of the claim.
telegram-confirm-text = Verknüpfen Sie { $name } (Telegram-ID { $id }, angefragt { $time }) nur, wenn das Ihr eigenes Telegram-Konto ist. Namen können täuschend ähnlich sein, prüfen Sie auch die ID. Danach handelt dieses Konto in tada als Sie.
telegram-confirm-submit = Verknüpfen
telegram-confirm-cancel = Abbrechen
telegram-linked = Telegram-Konto verknüpft.
# $id: the Telegram user ID. $time: the date and time of the link.
telegram-linked-account = Ihr tada-Konto ist mit dem Telegram-Konto { $id } verknüpft, seit { $time }.
telegram-unlink = Verknüpfung aufheben
telegram-unlink-title = Telegram-Verknüpfung aufheben?
telegram-unlink-text = Danach handelt dieses Telegram-Konto nicht mehr als Sie. Sie können später ein Telegram-Konto neu verknüpfen.
telegram-unlinked = Telegram-Verknüpfung aufgehoben.
telegram-requests-refreshed = Anfragen aktualisiert.

# The notice of ADR 0045. A changed text gets a new NOTICE_VERSION and a new line in NOTICE_TEXTS in crates/app/src/tokens/mod.rs; a test checks it.
token-notice-title = Hinweis zum API-Token
token-notice-access = Mit dem Token erhält Ihr eigener KI-Client Zugriff auf die Personendaten, die Sie in tada sehen können.
token-notice-policy = Die Richtlinie des Vereins verlangt einen KI-Tarif, der die eingegebenen Daten nicht zum Training verwendet.
token-notice-confirm = Ich habe den Hinweis gelesen und bestätige ihn.

## API tokens and the organization switch (settings)

tokens-title = API-Token
tokens-intro = Mit einem API-Token verbindet sich Ihr eigener KI-Client über MCP mit tada.
tokens-off = Die Organisationsleitung hat die MCP-Token ausgeschaltet. Sie können im Moment keinen Token erstellen.
tokens-create-title = Neuer Token
tokens-name = Name
tokens-name-help = Zum Beispiel der Name Ihres KI-Clients.
tokens-scope = Berechtigung
tokens-scope-read = Lesen
tokens-scope-propose = Lesen und Vorschläge machen
tokens-expiry = Gültig für
tokens-expiry-30 = 30 Tage
tokens-expiry-90 = 90 Tage
tokens-expiry-180 = 180 Tage
tokens-expiry-364 = Ein Jahr
tokens-create = Token erstellen
tokens-error-name = Geben Sie dem Token einen Namen.
tokens-error-name-too-long = Der Name ist zu lang. Kürzen Sie ihn.
tokens-error-notice = Der Hinweis hat sich geändert. Laden Sie die Seite neu und lesen Sie ihn erneut.
tokens-propose-forbidden = Das Recht, Vorschläge zu machen, fehlt Ihnen in allen Veranstaltungen. Wählen Sie „Lesen“.
tokens-created = Token erstellt.
tokens-secret-label = Ihr neuer Token
tokens-secret-once = Wird nur einmal angezeigt
tokens-secret-hint = Kopieren Sie den Token jetzt. tada kann ihn später nicht mehr zeigen.
tokens-copy = Token kopieren
tokens-copied = Token kopiert.
tokens-copy-failed = Kopieren nicht möglich. Markieren Sie den Token von Hand.
tokens-list-title = Ihre Token
tokens-loading = Token werden geladen
tokens-empty-title = Noch keine Token
tokens-empty-text = Erstellen Sie oben Ihren ersten Token.
tokens-column-name = Name
tokens-column-scope = Berechtigung
tokens-column-expires = Gültig bis
tokens-column-last-used = Zuletzt benutzt
tokens-column-status = Status
tokens-column-actions = Aktionen
tokens-never-used = Nie
tokens-status-active = Aktiv
tokens-status-revoked = Widerrufen
tokens-status-expired = Abgelaufen
tokens-revoke = Widerrufen
# $name: the name of the token.
tokens-revoke-of = { $name } widerrufen
tokens-revoke-title = Token widerrufen?
# $name: the name of the token.
tokens-revoke-text = Der Token „{ $name }“ funktioniert danach nicht mehr. Ein KI-Client mit diesem Token verliert den Zugriff.
tokens-revoke-cancel = Abbrechen
tokens-revoked = Token widerrufen.
tokens-example-title = Beispiel für einen MCP-Client
tokens-example-intro = Tragen Sie den Token als Bearer-Token ein. Ersetzen Sie TOKEN durch Ihren Token.
tokens-example-claude = Claude Code
tokens-example-codex = Codex
tokens-example-codex-text = Legen Sie den Token in die Umgebungsvariable TADA_TOKEN und tragen Sie in die Datei config.toml ein:

org-title = Organisation
org-mcp-label = MCP-Token erlauben
org-mcp-help = Nur wenn der Schalter an ist, können Mitglieder API-Token für KI-Clients erstellen. Nur die Organisationsleitung ändert ihn.
org-mcp-loading = Einstellungen werden geladen
org-mcp-on = MCP-Token sind eingeschaltet.
org-mcp-off = MCP-Token sind ausgeschaltet.

## The edit form of a value (ADR 0049)

value-error-required = Geben Sie einen Wert ein.
value-error-number = Das ist keine Zahl. Geben Sie eine Zahl wie 20000 oder 12,5 ein.
value-error-money = Das ist kein Betrag. Geben Sie einen Betrag wie 15.00 ein.
value-error-date = Das ist kein gültiges Datum. Geben Sie ein Datum wie 18.05.2030 ein.
value-error-range = Der obere Wert liegt unter dem unteren. Geben Sie einen Wert ab dem unteren Wert ein.
value-error-window = Das Ende liegt vor dem Beginn. Wählen Sie ein Datum ab dem Beginn.
value-error-granularity = Wählen Sie, wie genau das Zeitfenster ist.
value-error-choice = Wählen Sie mindestens eine Möglichkeit.
value-error-choice-single = Wählen Sie genau eine Möglichkeit.
value-input-text = Text
value-input-flag = Antwort
value-input-min = Wert
value-input-min-range = Von
value-input-max = Bis (leer lassen, wenn es ein einzelner Wert ist)
value-input-amount = Betrag in { $currency }
value-input-amount-max = Bis (leer lassen, wenn es ein einzelner Betrag ist)
value-input-date = Datum
value-input-start = Beginn
value-input-end = Ende
value-input-granularity = Genauigkeit
value-input-granularity-day = Tag
value-input-granularity-week = Woche
value-input-granularity-month = Monat
value-input-choice = Auswahl
value-input-approximate = Ungefährer Wert

## Review Inbox (ADR 0050)

inbox-title = Eingang
inbox-list-label = Offene Änderungen
inbox-loading = Eingang wird geladen
inbox-empty-title = Nichts zu prüfen
inbox-empty-text = Es gibt keine offenen Vorschläge. Neue Vorschläge von Mitgliedern und KI-Clients erscheinen hier.
inbox-more = Es gibt weitere offene Änderungen. Die Liste zeigt die ältesten zuerst.
inbox-scope-organization = Organisation
inbox-event-unknown = Anlass
# $count: the number of open proposals of the changeset.
inbox-open-proposals =
    { $count ->
        [one] 1 offener Vorschlag
       *[other] { $count } offene Vorschläge
    }
inbox-stale = Veraltet
inbox-stale-hint = Älter als 14 Tage
# $author: who proposed the change.
inbox-proposed-by = Von { $author }
inbox-author-member = Mitglied
# $time: a date and time.
inbox-proposed-at = vorgeschlagen am { $time }
inbox-detail-select = Wählen Sie in der Liste einen Eintrag.
# $title: the event or the organization of the changeset.
inbox-detail-of = Vorschläge zu { $title }
inbox-detail-loading = Vorschläge werden geladen
inbox-back = Zur Liste
# A changeset without the member's own words: its proposals cite stored sources only (ADR 0079).
inbox-no-source-text = Das Mitglied hat keine neuen Worte geschrieben. Die Vorschläge stützen sich nur auf gespeicherte Quellen.
inbox-event-closed = Dieser Anlass ist abgeschlossen. Seine Vorschläge lassen sich nicht mehr prüfen.
inbox-hints = Tastenkürzel
inbox-hint-j = nächster Eintrag
inbox-hint-k = vorheriger Eintrag
inbox-hint-a = annehmen
inbox-hint-e = bearbeiten
inbox-hint-r = ablehnen
inbox-active = Tastenkürzel gelten für diesen Vorschlag
inbox-list-position = Eintrag { $position } von { $total }

## Review Inbox: one proposal

# $title: the title of the proposal.
inbox-select = { $title } auswählen
inbox-section-change = Änderung
inbox-section-source = Quelle
inbox-section-reason = Begründung und Annahmen
inbox-current = Aktuell
inbox-proposed = Vorgeschlagen
inbox-reason = Begründung
inbox-assumption-note = Der Vorschlag gilt als Annahme. Er ist nicht bestätigt.
inbox-unknown-note = Der Vorschlag setzt den Wert auf „unbekannt“.
inbox-approximate-note = Der vorgeschlagene Wert ist ungefähr.
inbox-no-assumptions = Keine Annahmen.
inbox-source-none = Der Vorschlag nennt keine Quelle.
# $id: the start of the ID of the source version.
inbox-source-version = Quellversion { $id }
# $page: the page number in a PDF.
inbox-source-page = Seite { $page }
inbox-status-accepted = Angenommen
inbox-status-accepted-with-edit = Mit Änderung angenommen
inbox-status-rejected = Abgelehnt
inbox-status-withdrawn = Zurückgezogen
# Proposals that a legal redaction emptied (ADR 0072).
inbox-redacted-title = Entfernte Vorschläge
inbox-accept = Annehmen
inbox-edit = Bearbeiten und annehmen
inbox-reject = Ablehnen
inbox-conflict-fact-changed = Jemand hat den Wert nach dem Vorschlag geändert. Lehnen Sie den Vorschlag ab. Wer ihn gemacht hat, kann einen neuen Vorschlag zum aktuellen Wert machen.
inbox-conflict-target-changed = Der Eintrag hat sich nach dem Vorschlag geändert. Lehnen Sie den Vorschlag ab. Wer ihn gemacht hat, kann einen neuen Vorschlag machen.
inbox-conflict-dependency = Ein Vorschlag, von dem dieser abhängt, hat einen Konflikt. Lehnen Sie beide ab.
# $titles: the titles of the proposals, separated by commas.
inbox-depends-on = Setzt voraus: { $titles }
# $titles: the titles of the selected proposals that need this one.
inbox-needed-by = Mitgewählt, weil es gebraucht wird von: { $titles }

## Review Inbox: the operations of a proposal

inbox-op-create-event = Anlass anlegen
# $label: the label of the field.
inbox-op-set-fact = Wert für „{ $label }“
# $label: the label of the new field.
inbox-op-add-field = Feld „{ $label }“ hinzufügen
# $label: the label of the new choice.
inbox-op-add-choice = Auswahl „{ $label }“ hinzufügen
# $label: the label of the field.
inbox-op-deprecate-field = Feld „{ $label }“ ausmustern
inbox-op-create-question = Offene Frage anlegen
# $name: the name of the new document.
inbox-op-create-draft = Dokumentenentwurf „{ $name }“
inbox-op-create-draft-existing = Entwurf für ein bestehendes Dokument
inbox-op-unknown = Änderung ohne Darstellung
inbox-op-create-person = Person „{ $name }“ anlegen
inbox-op-create-institution = Institution „{ $name }“ anlegen
inbox-op-create-action = Aufgabe „{ $title }“ anlegen
inbox-op-create-commitment = Zusage anlegen
inbox-op-change-action-status = Status einer Aufgabe ändern
inbox-op-change-action-due = Fälligkeit einer Aufgabe ändern
inbox-op-change-commitment-status = Status einer Zusage ändern
# $title: the title of the new milestone.
inbox-op-create-milestone = Meilenstein „{ $title }“ anlegen
inbox-op-change-milestone-status = Status eines Meilensteins ändern
inbox-op-change-action-milestone = Meilenstein einer Aufgabe ändern
# $name: the name of the new workstream.
inbox-op-create-workstream = Arbeitsbereich „{ $name }“ anlegen
inbox-op-create-decision = Entscheid anlegen
inbox-op-supersede-decision = Entscheid ersetzen
inbox-op-revoke-decision = Entscheid widerrufen
inbox-row-wording = Wortlaut
inbox-row-decided-on = Beschlossen am
inbox-row-decided-by = Beschlossen von
inbox-row-decision = Entscheid
inbox-row-successor = Neuer Entscheid
inbox-row-reason = Grund
inbox-edit-decision-fixed = Wortlaut, Datum und Parteien bleiben, wie sie sind. Lehnen Sie den Vorschlag ab, wenn sie nicht stimmen.
inbox-milestone-removed = Kein Meilenstein
inbox-row-date = Datum
inbox-row-start = Startdatum
inbox-row-milestone = Meilenstein
inbox-row-new-milestone = Neuer Meilenstein
inbox-row-lead = Arbeitsbereichsleitung
inbox-name-unknown = Nicht lesbar
inbox-due-removed = Keine Fälligkeit
inbox-row-email = E-Mail
inbox-row-phone = Telefon
inbox-row-institution-kind = Art
inbox-row-title = Titel
inbox-row-owner = Verantwortlich
inbox-row-workstream = Arbeitsbereich
inbox-row-due = Fällig
inbox-row-commitment = Zusage
inbox-row-promisor = Zusagende Partei
inbox-row-condition = Bedingung
inbox-row-action = Aufgabe
inbox-row-new-status = Neuer Status
inbox-row-new-due = Neue Fälligkeit
inbox-overdue = Überfällig
inbox-other-review = Andere Prüfung
inbox-duplicates = Ähnliche Einträge
inbox-use-existing = Bestehenden Eintrag verwenden: { $name } ({ $id })
inbox-link-dependents-added =
    { $count ->
        [one] 1 Vorschlag, der den Eintrag braucht, mitgewählt.
       *[other] { $count } Vorschläge, die den Eintrag brauchen, mitgewählt.
    }
inbox-edit-error-condition = Die Bedingung hat 1 bis 500 Zeichen.
inbox-edit-error-condition-condition-fixed = Die Bedingung bleibt bestehen. Nur „Verbindlich machen“ beendet sie.
inbox-edit-error-email = Die E-Mail-Adresse ist ungültig.
inbox-edit-error-phone = Die Telefonnummer hat 1 bis 50 Zeichen.
inbox-link-other-reviewer = Ein anderer Prüfer muss die Vorschläge annehmen, die diesen Eintrag brauchen. Darum kann hier kein bestehender Eintrag verwendet werden.
inbox-link-invalid = Dieser Eintrag passt nicht zum Vorschlag. Wählen Sie einen anderen oder legen Sie die Person oder Institution neu an.
inbox-link-dependents-missing = Wählen Sie auch die Vorschläge, die den bestehenden Eintrag brauchen.
inbox-field-unknown = Feld
inbox-row-key = Kürzel
inbox-row-name = Name
inbox-row-time-zone = Zeitzone
inbox-row-field = Feld
inbox-row-label = Bezeichnung
inbox-row-field-key = Schlüssel
inbox-row-value-type = Wertetyp
inbox-row-description = Beschreibung
inbox-row-text = Text
inbox-value-type-text = Text
inbox-value-type-boolean = Ja oder Nein
inbox-value-type-quantity = Menge
inbox-value-type-money = Betrag
inbox-value-type-date = Datum
inbox-value-type-date-window = Zeitfenster
inbox-value-type-choice = Auswahl
inbox-value-type-reference = Verweis
inbox-draft-new = Das Dokument entsteht, wenn Sie den Entwurf annehmen.
inbox-draft-existing = Der Entwurf wird eine neue Version des Dokuments.
inbox-draft-open = Dokument öffnen
# $count: the number of lint warnings of the draft.
inbox-draft-warnings =
    { $count ->
        [one] 1 Hinweis der Prüfung
       *[other] { $count } Hinweise der Prüfung
    }

## Review Inbox: the selection and the actions

inbox-summary-title = Auswahl
# $count: the number of selected proposals.
inbox-summary-count =
    { $count ->
        [one] 1 Vorschlag ausgewählt
       *[other] { $count } Vorschläge ausgewählt
    }
# $count: the number of selected proposals that other selected proposals need.
inbox-summary-dependencies =
    { $count ->
        [one] Davon 1 als Abhängigkeit
       *[other] Davon { $count } als Abhängigkeiten
    }
inbox-summary-accept = Auswahl annehmen
inbox-summary-reject = Auswahl ablehnen
inbox-apply-blocked = Dieser Vorschlag hat einen Konflikt. Er lässt sich nicht annehmen.
inbox-summary-blocked = Die Auswahl enthält einen Konflikt. Sie lässt sich nicht annehmen.
# $count: the number of dependencies that the selection added.
inbox-dependencies-added =
    { $count ->
        [one] 1 Abhängigkeit mitgewählt.
       *[other] { $count } Abhängigkeiten mitgewählt.
    }
# $count: the number of proposals that the deselection removed.
inbox-dependents-removed =
    { $count ->
        [one] 1 abhängiger Vorschlag abgewählt.
       *[other] { $count } abhängige Vorschläge abgewählt.
    }
# $count: the number of accepted proposals.
inbox-applied =
    { $count ->
        [one] 1 Vorschlag angenommen.
       *[other] { $count } Vorschläge angenommen.
    }
# $count: the number of rejected proposals.
inbox-rejected =
    { $count ->
        [one] 1 Vorschlag abgelehnt.
       *[other] { $count } Vorschläge abgelehnt.
    }
inbox-reject-title = Vorschläge ablehnen?
inbox-reject-text = Abgelehnte Vorschläge lassen sich nicht wieder öffnen. Vorschläge, die davon abhängen, lehnt tada auch ab.
inbox-reject-cancel = Abbrechen
inbox-reject-confirm = Ablehnen
inbox-edit-title = Wert bearbeiten
inbox-edit-assumption = Als Annahme übernehmen
inbox-edit-cancel = Abbrechen
inbox-edit-submit = Bearbeiten und annehmen
inbox-edit-unavailable = Diesen Wert können Sie hier nicht bearbeiten.

privacy-title = Datenschutz
privacy-loading = Datenschutzerklärung wird geladen

org-privacy-title = Datenschutzerklärung
org-privacy-loading = Datenschutzerklärung wird geladen
org-privacy-label = Text der Datenschutzerklärung
org-privacy-help = Markdown. Ersetzen Sie jeden Platzhalter in eckigen Klammern. Alle Mitglieder und alle Eingeladenen lesen diesen Text.
org-privacy-template = Es gilt die Vorlage. Der Text unten ist die Vorlage. Mit „Speichern“ wird er der Text der Organisation.
org-privacy-save = Speichern
org-privacy-saved = Datenschutzerklärung gespeichert.
org-privacy-reset = Vorlage wiederherstellen
org-privacy-reset-title = Vorlage wiederherstellen?
org-privacy-reset-text = Der eigene Text wird ersetzt. Danach gilt wieder die Vorlage.
org-privacy-reset-confirm = Vorlage wiederherstellen
org-privacy-reset-cancel = Abbrechen
org-privacy-reset-done = Es gilt wieder die Vorlage.
org-privacy-error-empty = Geben Sie einen Text ein.
org-privacy-error-too-long = Der Text ist zu lang. Erlaubt sind 20 000 Zeichen.
org-privacy-conflict = Jemand hat die Datenschutzerklärung inzwischen geändert. Laden Sie die Seite neu, um den neuen Text zu sehen. Kopieren Sie Ihren Text vorher.
org-privacy-owner-only = Nur die Organisationsleitung ändert die Datenschutzerklärung.
org-privacy-read = Datenschutzerklärung lesen

## Persons and institutions (ADR 0069)

persons-title = Personen
institutions-title = Institutionen
parties-search = Nach Name suchen
parties-loading = Einträge werden geladen
parties-empty-title = Noch keine Einträge erfasst
parties-empty-text = Erfassen Sie den ersten Eintrag mit dem Formular unter der Liste.
parties-no-match-title = Keine Treffer
parties-no-match-text = Ändern Sie den Suchbegriff.
parties-column-id = ID
parties-column-name = Name
parties-column-kind = Art
parties-column-email = E-Mail-Adresse
parties-column-phone = Telefon
parties-column-actions = Aktionen
parties-edit = Bearbeiten
# $name: the name of the record.
parties-edit-of = { $name } bearbeiten
# $name: the name of the saved record.
person-create-title = Person erfassen
institution-create-title = Institution erfassen
# $name: the name of the record.
party-change-title = „{ $name }“ bearbeiten
party-name = Name (Pflichtfeld)
party-kind = Art
party-email = E-Mail-Adresse
party-phone = Telefon
party-account = Konto
party-account-linked = Verknüpft
party-create = Erfassen
party-save = Speichern
party-cancel = Abbrechen
party-error-name = Der Name hat 1 bis 200 Zeichen.
party-error-email = Das ist keine gültige E-Mail-Adresse.
party-error-phone = Die Telefonnummer hat 1 bis 50 Zeichen.
party-error-user_id = Dieses Mitglied gibt es nicht.
institution-kind-authority = Behörde
institution-kind-company = Firma
institution-kind-club = Verein
institution-kind-other = Andere

## Workstreams, actions and commitments (ADRs 0067 and 0068)

work-loading = Daten des Anlasses werden geladen
register-load-more = Weitere Einträge laden
register-loaded-more = Weitere Einträge geladen.
register-row-not-found = { $id } nicht gefunden
work-column-id = ID
work-column-owner = Verantwortlich
work-column-workstream = Arbeitsbereich
work-column-due = Fällig
work-column-status = Status
work-column-actions = Aktionen
work-edit = Bearbeiten
# $id: the readable ID of the record.
work-edit-of = { $id } bearbeiten
work-create = Erfassen
work-save = Speichern
work-cancel = Abbrechen
work-evidence = Beleg
# $id: the readable ID of the record.
work-evidence-of = Beleg zu { $id }
# $name: the readable ID or the name of the saved record.
register-saved = „{ $name }“ gespeichert.
register-conflict = Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen.
work-field-owner = Verantwortlich
work-field-workstream = Arbeitsbereich
work-field-start = Startdatum
work-field-due = Fällig am
work-field-milestone = Meilenstein
work-no-milestone = Kein Meilenstein
work-field-status = Status
work-no-workstream = Kein Arbeitsbereich
work-error-title = Der Titel hat 1 bis 200 Zeichen.
work-error-title-too-long = Der Titel hat höchstens 200 Zeichen.
work-error-description = Die Beschreibung hat 1 bis 4000 Zeichen.
work-error-mitigation = Die Gegenmassnahme hat 1 bis 4000 Zeichen.
work-error-impact = Wählen Sie eine Stufe für die Wirkung.
work-error-likelihood = Wählen Sie eine Stufe für die Wahrscheinlichkeit.
work-error-text = Der Text hat 1 bis 500 Zeichen.
work-error-condition = Die Bedingung hat 1 bis 500 Zeichen.
work-error-promisor = Wählen Sie eine Person oder eine Institution.
work-error-promisor-unknown-record = Diese Person oder Institution gibt es nicht.
work-error-owner = Wählen Sie ein Mitglied des Anlasses mit Mitarbeit oder Anlassleitung.
work-error-workstream = Wählen Sie einen aktiven Arbeitsbereich dieses Anlasses.
work-error-workstream-closed = Dieser Arbeitsbereich ist geschlossen.
work-error-due = Das ist kein gültiges Datum.
work-error-due_date = Das ist kein gültiges Datum.
work-error-date = Geben Sie ein gültiges Datum an.
work-error-start_date = Das ist kein gültiges Datum.
work-error-due_date-start-after-due = Fälligkeit liegt vor dem Start.
work-error-start_date-start-after-due = Das Startdatum liegt nach dem Fälligkeitsdatum. Wählen Sie ein früheres Startdatum.
work-error-milestone = Wählen Sie einen Meilenstein dieses Anlasses, der nicht abgesagt ist.
work-error-reason = Geben Sie einen Grund an. Er hat 1 bis 500 Zeichen.
work-error-reason-control-character = Der Grund enthält ein Zeichen, das nicht erlaubt ist.
work-error-name = Der Name hat 1 bis 200 Zeichen.
work-error-name-taken = Diesen Namen gibt es in diesem Anlass schon.
work-error-lead = Wählen Sie ein Mitglied des Anlasses mit Mitarbeit oder Anlassleitung.

actions-title = Aufgaben
actions-loading = Aufgaben werden geladen
actions-empty-title = Noch keine Aufgaben
actions-empty-text = Erfassen Sie die erste Aufgabe mit dem Formular unter der Liste.
action-column-title = Titel
action-create-title = Aufgabe erfassen
# $id: the readable ID of the action.
action-change-title = { $id } bearbeiten
action-field-title = Titel (Pflichtfeld)
action-field-description = Beschreibung
action-status-open = offen
action-status-in-progress = in Arbeit
action-status-blocked = blockiert
action-status-done = erledigt
action-status-canceled = abgebrochen

milestones-title = Meilensteine
milestones-loading = Meilensteine werden geladen
milestones-empty-title = Noch keine Meilensteine
milestones-empty-text = Erfassen Sie den ersten Meilenstein mit dem Formular unter der Liste.
milestone-column-title = Titel
milestone-column-date = Datum
milestone-column-open-actions = Offene Aufgaben
milestone-create-title = Meilenstein erfassen
# $id: the readable ID of the milestone.
milestone-change-title = { $id } bearbeiten
milestone-field-title = Titel (Pflichtfeld)
milestone-field-description = Beschreibung
milestone-field-date = Datum (Pflichtfeld)
milestone-status-open = offen
milestone-status-reached = erreicht
milestone-status-canceled = abgesagt
milestone-reach = Als erreicht markieren
# $id: the readable ID of the milestone.
milestone-reach-of = { $id } als erreicht markieren
milestone-reopen = Wieder öffnen
# $id: the readable ID of the milestone.
milestone-reopen-of = { $id } wieder öffnen
# $id: the readable ID of the milestone. $status: its new status, for example „erreicht“.
milestone-status-changed = { $id } ist jetzt { $status }.
milestone-warning-overdue = Datum überschritten
# $id: the readable ID of an open action that is due after the milestone.
milestone-warning-late-action = { $id } ist erst danach fällig
milestone-warning-late-unknown = Eine Aufgabe ist erst danach fällig
record-no-panel-title = Eintrag öffnet noch nicht hier
# $id: the readable ID from the address.
record-no-panel-text = Den Eintrag { $id } gibt es, aber er lässt sich noch nicht in diesem Fenster ansehen.
record-no-panel-open = Auf der bisherigen Seite öffnen
# $version: the record version that the evidence supports.
milestone-evidence-version = Stützt die Version { $version } des Meilensteins

## Decisions (ADR 0081)

decisions-title = Entscheide
decisions-loading = Entscheide werden geladen
decisions-empty-title = Noch keine Entscheide
decisions-empty-text = Erfassen Sie den ersten Entscheid mit dem Formular unter der Liste.
decisions-empty-text-read = Die Anlassleitung erfasst die Entscheide dieses Anlasses.
decision-column-wording = Wortlaut
decision-column-decided-on = Beschlossen am
decision-column-decided-by = Beschlossen von
decision-status-in-force = in Kraft
decision-status-superseded = ersetzt
decision-status-revoked = widerrufen
decision-superseded-by = ersetzt durch
# $reason: why the decision was revoked.
decision-revoked-reason-line = Grund: { $reason }
decision-create-title = Entscheid erfassen
# $id: the readable ID of the decision that the new one replaces.
decision-replace-title = { $id } ersetzen
# $id: the readable ID of the decision that the new one replaces.
decision-replace-gone = { $id } lässt sich nicht mehr ersetzen. Kopieren Sie den Text, und brechen Sie dann ab.
# $id: the readable ID of the decision that the new one replaces.
decision-replace-ended = { $id } lässt sich nicht mehr ersetzen.
# $id: the readable ID of the decision that the new one replaces.
decision-replace-stale = Die Fassung von { $id } ist nicht mehr aktuell. Kopieren Sie den Text, und brechen Sie dann ab. Öffnen Sie { $id } danach neu.
# $id: the readable ID of the decision that the new one replaces.
decision-replace-decided-on = Entschieden am
decision-replace-inherit = Verantwortlich und Bereich sind die des bisherigen Entscheids. Sie lassen sich hier ändern.
decision-replace-note = Der neue Entscheid ersetzt { $id }. Der Wortlaut von { $id } bleibt unverändert; { $id } gilt danach als ersetzt.
decision-field-wording = Wortlaut (Pflichtfeld)
decision-field-wording-help = Mehrere Zeilen sind möglich. Der Wortlaut lässt sich später nicht ändern. Ein neuer Entscheid ersetzt ihn. 1 bis 2000 Zeichen.
decision-field-decided-on = Beschlossen am (Pflichtfeld)
decision-field-decided-by = Beschlossen von (Pflichtfeld)
decision-parties-list = Beschlossen von
# $name: the name of a chosen person or institution.
parties-remove = { $name } entfernen
parties-remove-short = Entfernen
# $name: the name of the person or institution that the member chose.
parties-added = { $name } hinzugefügt
# $name: the name of the person or institution that the member removed.
parties-removed = { $name } entfernt
# $label: the name of the field, for example „Beschlossen von“.
people-picker-results = { $label }: Treffer
# $label: the name of the field.
people-picker-chosen = { $label }: gewählt
# $count: the people chosen. $max: the most that can be chosen.
people-picker-count = { $count } von { $max } gewählt
# $max: the most that can be chosen.
people-picker-full = Höchstens { $max } möglich.
decision-replace = Ersetzen
# $id: the readable ID of the decision.
decision-replace-of = { $id } ersetzen
decision-revoke = Widerrufen
# $id: the readable ID of the decision.
decision-revoke-of = { $id } widerrufen
# $id: the readable ID of the decision.
decision-revoke-title = { $id } widerrufen
decision-revoke-text = Der Entscheid gilt danach als widerrufen. Ein neuer Entscheid folgt nicht. Der Wortlaut und der Grund bleiben sichtbar.
decision-revoke-reason = Grund (Pflichtfeld)
decision-revoke-reason-help = Warum gilt der Entscheid nicht mehr? 1 bis 500 Zeichen.
decision-wording-more = Ganz zeigen
decision-wording-less = Kürzen
# $id: the readable ID of the decision.
decision-wording-more-of = Wortlaut von { $id } ganz zeigen
# $id: the readable ID of the decision.
decision-wording-less-of = Wortlaut von { $id } kürzen
# $id: the new decision. $old: the decision that it replaces.
decision-replaced = { $id } ersetzt { $old }.
# $id: the readable ID of the decision.
decision-revoked = { $id } ist jetzt widerrufen.
decision-evidence-version = Stützt die Version { $version } des Entscheids
decision-error-wording = Der Wortlaut hat 1 bis 2000 Zeichen.
decision-error-wording-too-long = Der Wortlaut hat höchstens 2000 Zeichen.
decision-error-wording-control-character = Der Wortlaut enthält ein Zeichen, das nicht erlaubt ist.
decision-error-decided_on = Geben Sie ein gültiges Datum an.
decision-error-decided_by = Wählen Sie 1 bis 50 Personen oder Institutionen.
decision-error-decided_by-too-long = Wählen Sie höchstens 50 Personen oder Institutionen.
decision-error-decided_by-unknown-record = Diese Person oder Institution gibt es nicht.
work-error-wording = { decision-error-wording }
work-error-wording-too-long = { decision-error-wording-too-long }
work-error-wording-control-character = { decision-error-wording-control-character }
work-error-decided_on = { decision-error-decided_on }
work-error-decided_by = { decision-error-decided_by }
work-error-decided_by-too-long = { decision-error-decided_by-too-long }
work-error-decided_by-unknown-record = { decision-error-decided_by-unknown-record }

timeline-title = Zeitplan
timeline-empty-title = Noch nichts im Zeitplan
timeline-empty-text = Geben Sie einer Aufgabe ein Fälligkeitsdatum oder erfassen Sie einen Meilenstein.
timeline-empty-action = Zu den Aufgaben
timeline-filters = Filter
timeline-filter-owner = Verantwortlich
timeline-filter-status = Status
timeline-filter-all = Alle
timeline-filter-reset = Filter zurücksetzen
timeline-no-match = Keine Treffer für diese Filter.
timeline-list-empty = Keine Einträge in dieser Ansicht.
# $count: the actions that the filters hide.
timeline-hidden = { $count ->
    [one] 1 Aufgabe ausgeblendet
   *[other] { $count } Aufgaben ausgeblendet
}
# $count: the actions without a due date. The link opens the register of actions.
timeline-undated = { $count ->
    [one] 1 Aufgabe ohne Fälligkeit
   *[other] { $count } Aufgaben ohne Fälligkeit
}
timeline-legend = Legende
timeline-today = heute
timeline-axis = Zeitachse
timeline-lane-none = ohne Arbeitsbereich
timeline-milestones = Meilensteine
timeline-by-week = Nach Wochen
# $date: the Monday of the week, for example 04.05.2030.
timeline-week = Woche vom { $date }
# $from and $to: the start date and the due date of the action.
timeline-span = { $from } bis { $to }
# $date: the due date of the action.
timeline-due = fällig { $date }
# $id, $title: the action. $when: its dates. $status: its status, for example „offen“.
timeline-action = { $id } { $title }, { $when }, { $status }
# $id, $title: the milestone. $date: its date. $status: its status.
timeline-milestone = { $id } { $title }, { $date }, { $status }

commitments-title = Zusagen
commitments-loading = Zusagen werden geladen
commitments-empty-title = Noch keine Zusagen
commitments-empty-text = Erfassen Sie die erste Zusage mit dem Formular unter der Liste.
commitment-column-text = Zusage
commitment-column-promisor = Zugesagt von
# $condition: the condition text of the commitment.
commitment-condition-line = Bedingung: { $condition }
# $reason: why the commitment became firm.
commitment-firm-reason-line = Verbindlich, weil: { $reason }
commitment-create-title = Zusage erfassen
# $id: the readable ID of the commitment.
commitment-change-title = { $id } bearbeiten
commitment-field-text = Zusage (Pflichtfeld)
commitment-field-promisor = Zugesagt von (Pflichtfeld)
commitment-field-promisor-placeholder = Person oder Institution wählen
commitment-field-promisor-fixed = Zugesagt von
commitment-field-condition = Bedingung
commitment-field-condition-help = Mit einer Bedingung ist die Zusage bedingt. Ohne Bedingung ist sie verbindlich.
commitment-field-condition-fixed = Bedingung
commitment-status-conditional = bedingt
commitment-status-firm = verbindlich
commitment-status-fulfilled = erfüllt
commitment-status-broken = gebrochen
commitment-status-withdrawn = zurückgezogen
# $version: the record version that the evidence supports.
commitment-evidence-version = Stützt die Version { $version } der Zusage
make-firm = Verbindlich machen
# $id: the readable ID of the commitment.
make-firm-of = { $id } verbindlich machen
# $id: the readable ID of the commitment.
make-firm-title = { $id } verbindlich machen
make-firm-text = Die Bedingung bleibt als Verlauf stehen. Der Grund wird mit der Zusage gespeichert.
make-firm-reason = Grund (Pflichtfeld)
make-firm-reason-help = Warum ist die Bedingung erfüllt?
make-firm-submit = Verbindlich machen
# $id: the readable ID of the commitment.
make-firm-done = { $id } ist jetzt verbindlich.

workstreams-title = Arbeitsbereiche
workstreams-loading = Arbeitsbereiche werden geladen
workstreams-empty-title = Noch keine Arbeitsbereiche
workstreams-empty-text = Die Anlassleitung legt den ersten Arbeitsbereich an.
workstream-column-name = Name
workstream-column-lead = Arbeitsbereichsleitung
workstream-create-title = Arbeitsbereich anlegen
# $name: the name of the workstream.
workstream-change-title = „{ $name }“ bearbeiten
# $name: the name of the workstream.
workstream-edit-of = { $name } bearbeiten
workstream-field-name = Name (Pflichtfeld)
workstream-field-lead = Arbeitsbereichsleitung
workstream-status-active = aktiv
workstream-status-closed = geschlossen

## Search lists

combobox-loading = Suche läuft
combobox-empty = Keine Treffer
search-select-search = Optionen durchsuchen
search-field-clear = Suche leeren

## My Work

my-work-title = Meine Arbeit
my-work-loading = Meine Arbeit wird geladen
my-work-empty-title = Nichts offen
my-work-empty-text = Sie haben keine offenen Aufgaben und Zusagen. Neue Einträge, die Ihnen gehören, erscheinen hier.
# $count: the number of proposals that the member reviews.
my-work-review =
    { $count ->
        [one] 1 Vorschlag wartet auf Ihre Prüfung
       *[other] { $count } Vorschläge warten auf Ihre Prüfung
    }

## Legal redaction, dispute notes and the Datenschutz section of a person (ADRs 0072 and 0073).

# $text: the objection of an owner about the shown evidence.
dispute-note = Einwand: { $text }
problem-content-redacted = Dieser Inhalt wurde entfernt.

## Retention periods of the organization (ADR 0071)

org-retention-title = Aufbewahrung
org-retention-loading = Fristen werden geladen
org-retention-intro = Nach Ablauf einer Frist entfernt tada den Inhalt automatisch. Ein leeres Feld heisst „keine automatische Löschung“.
org-retention-accounting = Quellen können Rechnungen und Verträge enthalten. Dafür gilt eine Aufbewahrungspflicht von 10 Jahren (OR Art. 958f). Legen Sie eine Frist erst fest, wenn Sie das geprüft haben.
org-retention-closed-label = Abgeschlossene Anlässe (Monate)
org-retention-closed-help = Der Inhalt eines Anlasses wird so viele Monate nach dem Abschluss entfernt. 1 bis 120. Leer: keine automatische Löschung.
org-retention-source-label = Quellen ohne Anlass (Monate)
org-retention-source-help = Der Inhalt einer Quelle ohne Anlass wird so viele Monate nach dem Hochladen entfernt. 1 bis 120. Leer: keine automatische Löschung.
org-retention-save = Speichern
org-retention-saved = Fristen gespeichert.
org-retention-error-out-of-range = Geben Sie eine Zahl von 1 bis 120 ein.
org-retention-conflict = Jemand hat die Fristen inzwischen geändert. Das Formular zeigt jetzt die neuen Werte.
org-retention-owner-only = Nur die Organisationsleitung ändert die Fristen.
org-retention-owner-only-hidden = Nur die Organisationsleitung sieht und ändert die Fristen.

## The „Arbeit“ page: all work records of an event (ADR 0082)

work-title = Arbeit
work-loading-board = Arbeit wird geladen
work-type-label = Typ
work-type-all = Alle
work-type-action = Aufgaben
work-type-commitment = Zusagen
work-type-milestone = Meilensteine
work-type-decision = Entscheide
work-type-risk = Risiken
work-search = Arbeit durchsuchen
work-search-placeholder = Titel oder ID
work-owner = Verantwortlich
work-owner-any = Alle Personen
work-owner-me = Ich
work-stream = Bereich
work-stream-any = Alle Bereiche
work-due = Fällig
work-due-any = Alle Termine
work-due-overdue = Überfällig
work-due-week = Diese Woche
work-due-none = Ohne Datum
work-filter-reset = Filter zurücksetzen
work-filter-open = Filter
# $count: the number of filters that are set.
work-filter-open-active = Filter, { $count } gesetzt
work-filter-title = Filter
# $count: the number of work records that the filters let through.
work-count =
    { $count ->
        [one] 1 Eintrag
       *[other] { $count } Einträge
    }
work-view-label = Ansicht
work-view-board = Board
work-view-table = Tabelle
work-view-timeline = Zeitplan
work-table-label = Arbeit als Tabelle
work-table-id = ID
work-table-kind = Typ
work-table-title = Titel
work-table-status = Status
work-table-owner = Verantwortlich
work-table-due = Fällig
work-table-area = Bereich
# The column with the checkboxes: its name for assistive technology.
work-table-select = Auswahl
# $column: the name of the column. $direction: asc or desc.
work-sorted =
    Sortiert nach { $column }, { $direction ->
        [asc] aufsteigend
       *[desc] absteigend
    }
work-timeline-only = Diese Ansicht zeigt Aufgaben und Meilensteine.
work-timeline-only-title = Nichts für den Zeitplan
work-timeline-failed = Daten des Anlasses konnten nicht geladen werden.
work-board-status = Status
work-board-column-open = Offen
work-board-column-in-progress = In Arbeit
work-board-column-blocked = Blockiert
work-board-column-done = Erledigt
work-board-column-canceled = Abgebrochen
work-board-column-empty = Keine Einträge
# $kind: the plural name of the kind, for example „Aufgaben“.
work-kind-failed = { $kind } konnten nicht geladen werden.
work-directory-failed = Personen und Bereiche konnten nicht geladen werden.
work-retry-failed = Erneut fehlgeschlagen
work-empty-title = Noch keine Arbeit in diesem Anlass
work-empty-text = Aufgaben, Zusagen, Meilensteine, Entscheide und Risiken erscheinen hier, sobald jemand sie erfasst.
work-empty-new-action = Neue Aufgabe
work-empty-filtered-title = Kein Eintrag passt zu den Filtern
work-empty-filtered-text = Ändern Sie die Filter oder setzen Sie sie zurück.
# $name: the name of the person who owns the record.
work-card-owner = Verantwortlich: { $name }
work-card-due = Fällig
work-card-decided = Entschieden am
# $level: niedrig, mittel or hoch.
work-card-impact = Wirkung { $level }
# $level: niedrig, mittel or hoch.
work-card-likelihood = Wahrscheinlich { $level }
# $id: the readable ID, $title: the title of the record. The name of the checkbox on a card.
work-select-item = { $id } { $title } auswählen
work-select-column = Alle in Spalte auswählen
work-select-all = Alle auswählen
work-bulk-label = Aktionen für die Auswahl
# $count: the number of selected records.
work-bulk-count = { $count } ausgewählt
work-bulk-status = Status
work-bulk-owner = Verantwortlich
work-bulk-area = Bereich
work-bulk-no-area = Kein Bereich
work-bulk-clear = Auswahl aufheben
# $done and $total: the records that are changed and all that run.
work-bulk-progress = { $done } von { $total } geändert …
# $count: the number of records that are changed.
work-bulk-done =
    { $count ->
        [one] 1 Eintrag geändert
       *[other] { $count } Einträge geändert
    }
work-bulk-nothing = Keine Änderung nötig
work-bulk-busy = Die vorige Änderung läuft noch.
# $count: the records that failed, $total: all records of the run.
work-bulk-failed-title =
    { $count ->
        [one] 1 von { $total } Einträgen nicht geändert
       *[other] { $count } von { $total } Einträgen nicht geändert
    }
# $count: the records that failed.
work-bulk-failed =
    { $count ->
        [one] 1 Eintrag konnte nicht geändert werden.
       *[other] { $count } Einträge konnten nicht geändert werden.
    }
work-bulk-failures = Nicht geändert
work-bulk-reason-conflict = Konflikt
work-bulk-reason-forbidden = keine Berechtigung
work-bulk-reason-not-found = nicht mehr vorhanden
work-bulk-reason-validation = Wert nicht angenommen
work-bulk-reason-unknown = unerwarteter Fehler
work-bulk-reason-directory = Personen und Bereiche sind noch nicht geladen.
work-bulk-reason-unsupported = Für einen Teil der Auswahl nicht möglich.
work-bulk-reason-status-mixed = Der Status lässt sich nur für Einträge derselben Art ändern.
work-bulk-reason-status-no-command = Der Status eines Entscheids ändert sich nur durch Ersetzen oder Widerrufen.
work-bulk-reason-status-none-common = Die ausgewählten Einträge haben keinen gemeinsamen nächsten Status.

## The record panel of the „Arbeit“ page (ADR 0082)

record-field-title = Titel
record-field-status = Status
record-field-owner = Verantwortlich
record-field-due = Fällig
record-field-date = Datum
record-field-wording = Wortlaut
record-field-decided-on = Entschieden
record-field-parties = Beteiligte
record-field-successor = Ersetzt durch
record-field-revoked-reason = Grund des Widerrufs
record-field-start = Startdatum
record-field-area = Bereich
record-field-milestone = Meilenstein
record-field-description = Beschreibung
record-field-impact = Wirkung
record-field-likelihood = Wahrscheinlichkeit
record-field-mitigation = Gegenmassnahme
record-no-date = Kein Datum
record-no-area = Kein Bereich
record-no-milestone = Kein Meilenstein
record-no-description = Keine Beschreibung
record-no-mitigation = Keine Gegenmassnahme
record-unknown-area = Unbekannter Bereich
record-unknown-milestone = Unbekannter Meilenstein
record-unknown-record = Unbekannter Eintrag
record-sources = Quellen
record-hints = Hinweise
record-not-found-title = Eintrag nicht gefunden
# $id: the readable ID from the address.
record-not-found-text = Den Eintrag { $id } gibt es in diesem Anlass nicht.
# $version: the record version that the evidence supports.
action-evidence-version = Stützt die Version { $version } der Aufgabe
# $version: the record version that the evidence supports.
risk-evidence-version = Stützt die Version { $version } des Risikos
# $field: the name of the field.
record-edit-field = { $field } bearbeiten
# $field: the name of the field. A button names itself by this, then by its visible value.
record-field-name = { $field }:
record-saving = Speichert …
# $field: the name of the field. $id: the readable ID of the record.
record-saved = { $field } von { $id } gespeichert.
# $message: why the field was not saved.
record-save-failed = { $field } von { $id } nicht gespeichert. { $message }
record-conflict = Der Eintrag wurde inzwischen geändert und ist neu geladen. Ihre Eingabe ist noch da. Speichern Sie sie erneut, wenn sie noch gilt.
# $value: what the record holds now, after another person changed it.
record-now-saved = Jetzt gespeichert: { $value }
record-save = Speichern
record-cancel = Abbrechen
record-retry = Erneut versuchen
record-discard = Verwerfen
record-create-action = Neue Aufgabe
record-create-risk = Neues Risiko
record-create-milestone = Neuer Meilenstein
record-create-decision = Neuer Entscheid
record-create-managers-only = Nur die Anlassleitung erfasst Entscheide.
record-create = Erstellen
record-create-forbidden = Sie können hier nichts erfassen. Das ist in einem abgeschlossenen Anlass und mit dem Lesezugriff nicht möglich.
record-create-unavailable = Die Personen sind nicht geladen, darum lässt sich hier nichts erfassen. Laden Sie die Seite neu.
record-choose-owner = Person wählen
record-choose-level = Stufe wählen
# $id: the readable ID of the new record.
record-created = { $id } erfasst.
work-new = Neu
record-required = (Pflichtfeld)
