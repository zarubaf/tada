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
shell-side = Seitenleiste
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
