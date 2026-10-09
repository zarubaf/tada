## Telegram bot, German (Switzerland). See ADRs 0005 and 0011.

telegram-help =
    Senden Sie mir den Verknüpfungscode aus tada. Sie finden ihn in tada unter „Telegram verknüpfen“.
    Mit „/vorschlag KÜRZEL FELD WERT“ schlagen Sie eine Änderung vor, zum Beispiel „/vorschlag FLY28 date_window 2030-05..2030-06“.
telegram-link-claimed = Danke. Bestätigen Sie die Verknüpfung jetzt in tada. Erst danach ist Ihr Telegram-Konto verknüpft.
telegram-link-invalid = Dieser Code ist ungültig oder abgelaufen. Erstellen Sie in tada einen neuen Code.
telegram-error = Ein Fehler ist aufgetreten. Versuchen Sie es in einigen Minuten erneut.

## The command /vorschlag

telegram-proposal-usage = So schlagen Sie eine Änderung vor: „/vorschlag KÜRZEL FELD WERT“. Zum Beispiel: „/vorschlag FLY28 date_window 2030-05..2030-06“.
telegram-proposal-created = Danke. Ihr Vorschlag für den Anlass { $event } ist erfasst. Die Anlassleitung prüft ihn in tada.
telegram-proposal-reason = Per Telegram vorgeschlagen.
telegram-not-linked = Ihr Telegram-Konto ist nicht verknüpft. Verknüpfen Sie es zuerst in tada unter „Telegram verknüpfen“.
telegram-ambiguous = Sie haben in mehreren Organisationen einen Anlass mit diesem Kürzel. Machen Sie den Vorschlag in tada.
telegram-unknown-field = Diesen Anlass gibt es, aber er hat kein offenes Feld mit diesem Schlüssel. Die Felder sehen Sie in tada.
telegram-value-invalid = Dieser Wert passt nicht zum Feld. Schreiben Sie Zahlen ohne Apostroph und ohne Leerzeichen. Beispiele: Text, ja oder nein, 20000, CHF 80000, 2030-05-18, 2030-05..2030-06.
telegram-value-web-only = Dieses Feld verweist auf einen anderen Eintrag. Ändern Sie es in tada.

## Problem codes (ADR 0037): `problem-<code>`

problem-malformed-request = Die Anfrage ist ungültig.
problem-unauthenticated = Sie sind nicht angemeldet.
problem-organization-required = Wählen Sie zuerst eine Organisation in tada.
problem-recent-sign-in-required = Melden Sie sich in tada neu an.
problem-forbidden = Sie haben keine Berechtigung für diese Aktion.
problem-not-found = Das gibt es nicht, oder Sie dürfen es nicht sehen.
problem-record-version-conflict = Jemand hat diesen Eintrag inzwischen geändert. Versuchen Sie es erneut.
problem-invalid-transition = Diese Änderung ist im aktuellen Zustand nicht möglich.
problem-payload-too-large = Die Nachricht ist zu lang.
problem-unsupported-media-type = Diese Art von Nachricht ist nicht erlaubt.
problem-validation-failed = Die Eingabe ist ungültig.
problem-rate-limited = Zu viele Anfragen. Versuchen Sie es in einigen Minuten erneut.
problem-unavailable = Der Dienst ist im Moment nicht erreichbar. Versuchen Sie es in einigen Minuten erneut.
problem-internal = Ein unerwarteter Fehler ist aufgetreten.
