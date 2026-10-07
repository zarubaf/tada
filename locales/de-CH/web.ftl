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
shell-side = Seitenleiste
shell-nav = Hauptnavigation
nav-events = Anlässe
organization-switcher = Organisation
member-menu = Mitgliedermenü
sign-out = Abmelden
