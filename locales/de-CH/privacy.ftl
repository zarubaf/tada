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

    Der Betreiber nennt dem Verein auf Anfrage die Dienstleister jeder Kategorie.

    # Wozu wir die Daten bearbeiten

    Wir planen damit Anlässe des Vereins:
    Mitglieder arbeiten gemeinsam an Fakten, Quellen, Dokumenten und Entwürfen.
    Wir melden Mitglieder an, senden ihnen E-Mails und prüfen, wer was sehen und ändern darf.

    # Welche Daten wir bearbeiten

    - Angaben zu Mitgliedern: Anzeigename, E-Mail-Adresse, Mitgliedschaften und Rollen.
    - Angaben zur Anmeldung: Sitzungen mit dem Browser, Einladungen und Anmeldelinks.
    - Zähler für Anmeldeversuche: pro E-Mail-Adresse und pro IP-Adresse, als verschlüsselter Schlüssel.
      Wir bewahren sie höchstens zwei Stunden auf.
    - Inhalte der Anlässe: Fakten, Vorschläge, Prüfungen, offene Fragen, Entwürfe und hochgeladene Dateien.
    - Quellentexte, zum Beispiel eingefügte E-Mails oder Notizen.
      Sie können Namen und Kontaktdaten von Dritten enthalten.
    - Ein Protokoll, wer was getan hat. Es enthält Kennungen und Rollen, keine Namen.
    - Angaben zu API-Token: Name, Zeitpunkt der Erstellung und der letzten Nutzung.

    Die Protokolle des Betriebs enthalten keine Namen, E-Mail-Adressen oder Textinhalte.

    # Telegram

    Telegram ist ein freiwilliger Kanal.
    Wer ihn nutzt, verknüpft sein Telegram-Konto mit tada.
    Dann speichert tada die Telegram-Kennung und den Namen des Kontos.
    Ohne diese Verknüpfung nutzt tada Telegram nicht.

    Ein verknüpftes Mitglied kann mit dem Befehl /vorschlag einen Wert für einen Anlass vorschlagen.
    tada speichert die ganze Nachricht als Quellentext beim Anlass.
    Alle, die den Anlass lesen dürfen, sehen diesen Text, auch über die KI-Clients der Mitglieder.
    Die Nachricht und die Antwort des Bots, zum Beispiel mit dem Namen des Anlasses, gehen über Telegram.

    # KI-Clients der Mitglieder

    Ein Mitglied kann ein API-Token erstellen und damit den eigenen KI-Client verbinden.
    Der KI-Client liest über Lesewerkzeuge (MCP) alles, was das Mitglied selbst sehen darf:
    Anlässe, Fakten, Quellen mit ihren Textstellen und Dokumente.
    Das können Personendaten sein.
    Der Verein verlangt einen KI-Tarif, der die Eingaben nicht zum Training nutzt.
    Die Organisationsleitung kann API-Token ausschalten.

    # Zitate als Belege

    Ein Vorschlag belegt seinen Inhalt mit einem Zitat aus einer Quelle.
    Das Zitat kann den ganzen Text einer Eingabe zeigen, die für die ganze Organisation gilt und nicht für einen einzelnen Anlass.
    Alle, die den Anlass lesen dürfen, sehen dann diesen Text.
    Das gilt in der Web-Oberfläche und für die KI-Clients der Mitglieder.

    # Hochgeladene Dateien

    tada prüft hochgeladene Dateien nicht auf Schadsoftware.
    Öffnen Sie Dateien nur, wenn Sie dem Absender vertrauen.

    # Aufbewahrung

    - Backups bewahren wir [Dauer der Backups] auf.
    - Protokolle des Betriebs bewahren wir [Dauer der Protokolle] auf.
    - Gelöschte Daten bleiben bis zum Ablauf der Backups in den Backups.

    # Ihre Rechte

    Sie haben das Recht auf Auskunft, auf Berichtigung und auf Löschung Ihrer Daten.
    Wir beantworten Ihre Anfrage innerhalb von 30 Tagen.
    Schreiben Sie an [Kontakt für Anfragen].
    Sie können sich auch an den Eidgenössischen Datenschutz- und Öffentlichkeitsbeauftragten wenden.
