# The template of the privacy notice (ADR 0045).
# An organization shows it until an owner writes an own text. The text is Markdown.
# Each placeholder is a text in square brackets. The operator and the club replace them.
# No line may start with a square bracket, a star or a dot: Fluent reads those as syntax.
privacy-template =
    # Verantwortlich

    Verantwortlich für die Daten in tada ist der Verein [Name des Vereins], [Adresse des Vereins].
    Der Verein ist der Verantwortliche im Sinn des Datenschutzgesetzes.

    # Wer die Daten bearbeitet

    Der Verein lässt tada durch einen Betreiber betreiben.
    Der Betreiber und seine Dienstleister bearbeiten die Daten im Auftrag des Vereins.
    Es gibt diese Kategorien von Auftragsbearbeitern:

    - Betreiber von tada
    - Hosting von Servern und Speicher
    - Backups
    - Versand von E-Mails
    - Modellanbieter für die KI-Formulierung der Erinnerungen, nur wenn die Organisation sie einschaltet

    Der Betreiber nennt dem Verein auf Anfrage die Dienstleister jeder Kategorie.

    # Wozu wir die Daten bearbeiten

    Wir planen damit Anlässe des Vereins:
    Mitglieder arbeiten gemeinsam an Fakten, Quellen, Dokumenten und Entwürfen.
    Wir melden Mitglieder an, senden ihnen E-Mails und prüfen, wer was sehen und ändern darf.

    # Welche Daten wir bearbeiten

    - Angaben zu Mitgliedern: Anzeigename, E-Mail-Adresse, Mitgliedschaften und Rollen.
    - Angaben zur Anmeldung: Sitzungen mit der Kennung des Browsers, Einladungen und Anmeldelinks.
    - Zähler für Anmeldeversuche: pro E-Mail-Adresse und pro IP-Adresse, als Prüfwert mit geheimem Schlüssel (HMAC).
      Wir bewahren sie höchstens zwei Stunden auf.
    - Inhalte der Anlässe: Fakten, Vorschläge, Prüfungen, offene Fragen, Entwürfe und hochgeladene Dateien.
    - Quellentexte, zum Beispiel eingefügte E-Mails oder Notizen.
      Sie können Namen und Kontaktdaten von Dritten enthalten.
    - Ein Protokoll, wer was getan hat. Es enthält Kennungen und Rollen, keine Namen.
    - Angaben zu API-Token: Name, Zeitpunkt der Erstellung und der letzten Nutzung.
    - Einstellungen für Erinnerungen: ob das Mitglied zugestimmt hat, und seine Zeitzone.
    - Erinnerungsstand: welche Aufgabe oder Zusage tada einem Mitglied gemeldet hat und was das Mitglied geantwortet hat.
    - Hinweise zu Daten, die eine Person bestreitet, mit Text, Autorin oder Autor und Zeitpunkt.
    - Ein Protokoll der Entfernungen: welche Inhalte wann und warum entfernt wurden, ohne die Inhalte selbst.

    Die Protokolle des Betriebs enthalten keine Namen, E-Mail-Adressen oder Textinhalte.

    # Telegram

    Telegram ist ein freiwilliger Kanal.
    Wer ihn nutzt, verknüpft sein Telegram-Konto mit tada.
    Dazu sendet das Konto dem Bot einen Code, und das Mitglied bestätigt die Verknüpfung in tada.
    Sobald ein Konto einen gültigen Code sendet, speichert tada dessen Telegram-Kennung und Namen, auch vor der Bestätigung.
    Ohne einen solchen Code nutzt tada Telegram nicht.

    Ein verknüpftes Mitglied kann mit dem Befehl /vorschlag einen Wert für einen Anlass vorschlagen.
    tada speichert die ganze Nachricht als Quellentext beim Anlass.
    Alle, die den Anlass lesen dürfen, sehen diesen Text, auch über die KI-Clients der Mitglieder.
    Die Nachricht und die Antwort des Bots, zum Beispiel mit dem Namen des Anlasses, gehen über Telegram.

    # Erinnerungen in Telegram

    Der AI-PM von tada prüft zu festen Zeiten, welche Aufgaben und Zusagen fällig, überfällig oder blockiert sind.
    Er sendet Erinnerungen nur an Mitglieder, die zugestimmt haben und ein bestätigtes Telegram-Konto verknüpft haben.
    Ohne Ihre Zustimmung sendet tada keine Erinnerungen.
    Sie können die Zustimmung jederzeit in tada zurücknehmen.

    Eine Erinnerung ist eine private Nachricht mit höchstens zehn Einträgen.
    Jeder Eintrag nennt den Titel, den Namen des Anlasses, die Frist, den Status und den Grund der Erinnerung.
    Der Grund ist überfällig, bald fällig, blockiert oder Nachfrage.
    Mit Schaltflächen können Sie antworten, zum Beispiel „Erledigt“ oder „Blockiert“.
    tada speichert, worüber es Sie informiert hat und was Sie geantwortet haben.
    tada löscht diese Angaben 90 Tage danach.

    Die Organisation legt Ruhezeiten fest.
    Als Voreinstellung sendet tada abends zwischen 20:00 und 08:00 Uhr nichts, gemessen in Ihrer Zeitzone.
    Eine Nachricht aus der Ruhezeit kommt am Ende der Ruhezeit.
    Dafür speichert tada Ihre Zeitzone.
    Die Nachrichten und Ihre Antworten gehen über Telegram.

    # KI-Formulierung der Erinnerungen

    Die Organisation kann einschalten, dass ein KI-Modell den Text der Erinnerungen formuliert.
    Diese Funktion ist ausgeschaltet, bis die Organisation sie einschaltet.
    Ist sie ausgeschaltet, nutzt tada eine feste Vorlage, und kein Modell sieht Ihre Daten.

    Ist sie eingeschaltet, sendet tada dem Modell nur diese Angaben zu den Einträgen:
    den Titel, den Namen des Anlasses, die Frist, den Status und den Grund der Erinnerung.
    Dazu kommen die Sprache und das heutige Datum.
    tada sendet keine Namen aus den Personendaten, keine E-Mail-Adressen und keine Quellentexte.
    Titel und Namen von Anlässen sind aber freier Text, den Mitglieder schreiben.
    Sie können Namen von Personen enthalten.
    Das Modell kann nur Text formulieren.
    Es ändert keine Daten, und die Schaltflächen stammen nicht vom Modell.
    Wenn das Modell ausfällt oder sein Text die Prüfung nicht besteht, sendet tada die Vorlage.

    Der Modellanbieter ist Anthropic.
    Er ist ein Dienstleister des Betreibers und bearbeitet die Daten im Auftrag des Vereins.
    Der Betreiber prüft den Anbieter vor dem ersten Aufruf:
    die Bedingungen der Bearbeitung, dass der Anbieter die Daten nicht zum Training nutzt, die Dauer der Aufbewahrung beim Anbieter, die Region der Bearbeitung und die Rechtsgrundlage für eine Übermittlung ins Ausland.
    Ohne diese Prüfung ruft tada kein Modell auf.
    tada speichert pro Aufruf die Aufgabe, die Version des Textes, die Kennung des Modells, die Zahl der Tokens und die Kosten.
    Diese Angaben enthalten keine Inhalte.
    Die Organisation hat eine monatliche Obergrenze für die Kosten.

    # KI-Clients der Mitglieder

    Ein Mitglied kann ein API-Token erstellen und damit den eigenen KI-Client verbinden.
    Der KI-Client liest über Lesewerkzeuge (MCP) alles, was das Mitglied selbst sehen darf:
    Anlässe, Fakten, Quellen mit ihren Textstellen und Dokumente.
    Das können Personendaten sein.
    Der KI-Client kann auch Vorschläge machen.
    Den Text eines solchen Vorschlags speichert tada als Quellentext.
    Der Verein verlangt einen KI-Tarif, der die Eingaben nicht zum Training nutzt.
    Die Organisationsleitung kann API-Token ausschalten.

    # Zitate als Belege

    Ein Vorschlag oder ein Entwurf belegt seinen Inhalt mit einem Zitat aus einer Quelle.
    Das Zitat kann den ganzen Text einer Eingabe zeigen, die für die ganze Organisation gilt und nicht für einen einzelnen Anlass.
    Alle, die den Anlass lesen dürfen, sehen dann diesen Text.
    Das gilt in der Web-Oberfläche und für die KI-Clients der Mitglieder.

    # Hochgeladene Dateien

    tada prüft hochgeladene Dateien nicht auf Schadsoftware.
    Öffnen Sie Dateien nur, wenn Sie dem Absender vertrauen.

    # Aufbewahrung

    - Die Organisation legt fest, wie lange tada abgeschlossene Anlässe und Quellen aufbewahrt.
      Ohne eine solche Festlegung löscht tada nichts.
      Nach Ablauf entfernt tada die Inhalte und behält nur die Struktur: Kennungen, Zustände, Daten und das Protokoll.
      Personen und Institutionen im Adressverzeichnis des Vereins entfernt die Organisationsleitung auf Anfrage.
    - Technische Daten löscht tada nach festen Fristen:
      Anmeldelinks und beendete Sitzungen nach 30 Tagen, Einladungen, Versandprotokolle und Erinnerungsstand nach 90 Tagen.
    - Das Konto einer Person anonymisiert tada 90 Tage, nachdem die letzte Mitgliedschaft geendet hat.
    - Backups bewahren wir [Dauer der Backups] auf.
    - Protokolle des Betriebs bewahren wir [Dauer der Protokolle] auf.
    - Gelöschte Daten bleiben bis zum Ablauf der Backups in den Backups.
    - Exporte der Daten bewahren wir wie Backups auf.

    # Entfernen von Daten und Hinweis auf Bestreitung

    Die Organisationsleitung kann auf Anfrage Inhalte für immer entfernen.
    Das betrifft Quellen, Fakten, Dokumentversionen, Vorschläge, Personen und Institutionen.
    tada ersetzt den Inhalt durch einen Vermerk „entfernt“.
    Kennungen und Verweise bleiben, damit die Struktur erhalten bleibt.
    tada speichert keinen Prüfwert der entfernten Daten.
    Das Protokoll nennt, wer was wann und warum entfernt hat, ohne den Inhalt.
    Ein Journal der Entfernungen liegt ausserhalb der Datenbank.
    Der Betreiber spielt es nach jeder Wiederherstellung eines Backups ein, bevor tada wieder online geht.
    So kommen entfernte Daten nicht zurück.

    Belege sind unveränderlich und lassen sich nicht berichtigen.
    Bestreitet eine Person eine Angabe, hält die Organisationsleitung das in einem Hinweis fest.
    Alle, die den Beleg lesen, sehen den Hinweis, auch über die KI-Clients der Mitglieder.
    Solche Hinweise sind auch in einem abgeschlossenen Anlass möglich.

    Nur die Organisationsleitung darf diese Funktionen nutzen.
    Sie braucht dafür eine Anmeldung, die höchstens 15 Minuten zurückliegt.
    KI-Clients können sie nicht nutzen.

    # Auskunft über eine Person

    Auf Anfrage exportiert die Organisationsleitung alle Daten zu einer Person als JSON-Datei.
    Dazu gehören die Erwähnungen in Texten, die Verweise und die Hinweise auf Bestreitung.
    Hat die Person ein Konto, gehören auch die Kontodaten dazu:
    Anzeigename, E-Mail-Adresse, Rollen, Telegram-Verknüpfung, Sitzungen, API-Token mit Name und Datum, Einladungen und Einträge im Protokoll.
    Die Datei enthält keine Geheimnisse.
    tada findet Erwähnungen über Name, E-Mail-Adresse und Telefonnummer.
    Andere Schreibweisen findet tada nicht.
    Die Organisationsleitung prüft die Liste, bevor sie Daten entfernt.

    # Ihre Rechte

    Sie haben das Recht auf Auskunft, auf Berichtigung und auf Löschung Ihrer Daten.
    Sie haben auch das Recht, Ihre Daten in einem gängigen Format zu erhalten.
    Wir beantworten Ihre Anfrage innerhalb von 30 Tagen.
    Schreiben Sie an [Kontakt für Anfragen].
    Sie können sich auch an den Eidgenössischen Datenschutz- und Öffentlichkeitsbeauftragten wenden.
