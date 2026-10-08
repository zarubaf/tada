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
problem-forbidden = Sie haben keine Berechtigung für diese Aktion.
problem-unavailable = Der Dienst ist im Moment nicht erreichbar. Versuchen Sie es in einigen Minuten erneut.
problem-general-4xx = Die Anfrage ist ungültig.
problem-general-5xx = Ein unerwarteter Fehler ist aufgetreten.
problem-network = Keine Verbindung zum Server. Prüfen Sie Ihre Internetverbindung.

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

## Navigation

not-found-title = Seite nicht gefunden
not-found-text = Diese Adresse gibt es nicht. Prüfen Sie den Link oder wählen Sie einen Eintrag in der Navigation.

## Session

session-loading = Sitzung wird geladen
skip-link = Zum Inhalt springen
shell-nav = Hauptnavigation
nav-events = Anlässe
organization-switcher = Organisation
member-menu = Mitgliedermenü
sign-out = Abmelden

## Sign-in, magic link and invitation (ADR 0008, ADR 0056)

sign-in-title = Anmelden
sign-in-text = Wir senden Ihnen einen Link per E-Mail. Mit dem Link melden Sie sich an.
sign-in-email = E-Mail-Adresse (Pflichtfeld)
sign-in-submit = Anmeldelink senden
sign-in-sent = Wenn die Adresse bekannt ist, erhalten Sie in Kürze eine E-Mail.
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
magic-link-submit = Anmelden
magic-link-invalid = Dieser Link ist ungültig oder abgelaufen.
to-sign-in = Zur Anmeldung
invitation-title = Einladung
# $organization: the name of the organization. $role: the role, for example „Mitglied“.
invitation-text = Sie sind eingeladen: Organisation { $organization }, Rolle { $role }.
invitation-accept = Einladung annehmen
invitation-invalid = Diese Einladung ist ungültig oder abgelaufen.
invitation-loading = Einladung wird geladen
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
event-overview-placeholder = Hier erscheint die Übersicht des Anlasses.
event-loading = Anlass wird geladen
problem-not-found = Das gibt es nicht, oder Sie dürfen es nicht sehen.
problem-validation-failed = Die Eingabe ist ungültig.
problem-record-version-conflict = Jemand hat diesen Eintrag inzwischen geändert. Versuchen Sie es erneut.
problem-invalid-transition = Diese Änderung ist im aktuellen Zustand nicht möglich.
problem-payload-too-large = Die Datei ist zu gross.
problem-unsupported-media-type = Dieser Dateityp ist nicht erlaubt.

## Documents

# The `validation-failed` entry `quota-exceeded` on `file`: the upload is over the storage quota of the organization (ADR 0043).
document-error-file-quota-exceeded = Der Speicherplatz der Organisation reicht für diese Datei nicht aus.

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
members-remove-text = { $name } verliert den Zugang zur Organisation und zu allen Anlässen.
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
members-leave-warning = Sie entfernen sich selbst. Danach sehen Sie diese Organisation nicht mehr und können nur über eine neue Einladung zurückkehren.
members-remove-last = Die Organisation braucht mindestens eine Organisationsleitung, und jeder Anlass braucht eine Anlassleitung. Bestimmen Sie zuerst jemand anderen.
members-conflict = Jemand hat die Mitglieder inzwischen geändert. Die Liste ist neu geladen. Versuchen Sie es erneut.
members-loaded-more = Weitere Mitglieder geladen.

## Settings navigation and Telegram link (settings)

settings-nav = Einstellungen
settings-nav-members = Mitglieder
settings-nav-telegram = Telegram
telegram-title = Telegram verknüpfen
telegram-intro = Verknüpfen Sie Ihr Telegram-Konto mit tada, um tada im Chat zu nutzen.
telegram-steps-title = So geht es
telegram-step-create = Erstellen Sie hier einen Code.
telegram-step-send = Senden Sie den Code dem Bot in Telegram.
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
telegram-requests-empty-text = Sobald der Bot Ihren Code erhält, erscheint hier das Telegram-Konto.
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
telegram-requests-refreshed = Anfragen aktualisiert.
