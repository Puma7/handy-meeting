using System.Windows;
using System.Windows.Controls;

namespace HandyMeeting;

internal static class TemplateEditor
{
    public static string? Name(Window owner, string title, string value)
    {
        var input = new TextBox { Text = value, MaxLength = 100, Margin = new(0, 8, 0, 16) };
        var panel = new StackPanel { Margin = new(22) };
        panel.Children.Add(new TextBlock { Text = title, FontSize = 18 }); panel.Children.Add(input);
        var window = new Window { Owner = owner, Title = Ui.T(title), Width = 480, SizeToContent = SizeToContent.Height, ResizeMode = ResizeMode.NoResize, WindowStartupLocation = WindowStartupLocation.CenterOwner, Content = panel };
        var save = new Button { Content = Ui.T("Speichern"), IsDefault = true, HorizontalAlignment = HorizontalAlignment.Right };
        save.Click += (_, _) => { if (!string.IsNullOrWhiteSpace(input.Text)) window.DialogResult = true; };
        panel.Children.Add(save); Ui.Attach(window);
        return window.ShowDialog() == true ? input.Text.Trim() : null;
    }
    public static AnalysisStep? Step(Window owner, AnalysisStep initial)
    {
        var name = new TextBox { Text = initial.Name, MaxLength = 100, Margin = new(0, 6, 0, 12) };
        var prompt = new TextBox { Text = initial.Prompt, MaxLength = 8000, AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Margin = new(0, 6, 0, 12), VerticalContentAlignment = VerticalAlignment.Top };
        var kinds = new ComboBox { ItemsSource = new[] { Ui.T("Eigene Auswertung"), Ui.T("Kurzer Überblick"), Ui.T("Beschlossene Entscheidungen"), Ui.T("Aufgaben mit Zuständigen und Terminen") }, SelectedIndex = (int)initial.Kind, Margin = new(0, 6, 0, 12) };
        var grid = new Grid { Margin = new(22) };
        foreach (var height in new[] { GridLength.Auto, GridLength.Auto, GridLength.Auto, GridLength.Auto, GridLength.Auto, new GridLength(1, GridUnitType.Star), GridLength.Auto, GridLength.Auto }) grid.RowDefinitions.Add(new() { Height = height });
        void Add(UIElement item, int row) { Grid.SetRow(item, row); grid.Children.Add(item); }
        Add(new TextBlock { Text = Ui.T("Name des Bereichs") }, 0); Add(name, 1);
        Add(new TextBlock { Text = Ui.T("Wie soll das Ergebnis aussehen?") }, 2); Add(kinds, 3);
        Add(new TextBlock { Text = Ui.T("Was soll die KI aus dem Gespräch herausarbeiten?") }, 4); Add(prompt, 5);
        prompt.ToolTip = Ui.T("Beschreibe Thema und Ausgabesprache. Beispiel: Nenne auf Englisch die angesprochenen Risiken und offenen Fragen. Ein Prompt ist der Arbeitsauftrag für diesen Bereich.");
        var explanation = new TextBlock { Margin = new(0, 4, 0, 14), FontSize = 12 };
        void Explain() => explanation.Text = Ui.T(kinds.SelectedIndex switch
        {
            1 => "Verdichtet die belegten Themen zu einem kurzen Gesamtüberblick. Zuständige und Termine gehören in Aufgaben.",
            2 => "Sucht ausdrücklich getroffene Beschlüsse. Wünsche, Vorschläge und Zustandsbeschreibungen gelten nicht als Entscheidung.",
            3 => "Sucht ausdrücklich zugesagte oder zugewiesene Aufgaben. Zuständige und Termine bleiben unbekannt, wenn sie nicht genannt wurden.",
            _ => "Für eigene Themen wie Risiken, offene Fragen oder Fachbegriffe. Jeder Punkt braucht ein wörtliches Originalzitat. Audiospuren erlauben noch keine genaue Personenstatistik."
        });
        kinds.SelectionChanged += (_, _) => Explain(); Explain(); Add(explanation, 6);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right };
        var cancel = new Button { Content = Ui.T("Abbrechen"), IsCancel = true };
        var save = new Button { Content = Ui.T("Speichern"), IsDefault = true };
        buttons.Children.Add(cancel); buttons.Children.Add(save); Add(buttons, 7);
        var window = new Window { Owner = owner, Title = Ui.T("Verarbeitungsschritt bearbeiten"), Width = 740, Height = 620, MinHeight = 500, MinWidth = 560, Content = grid, WindowStartupLocation = WindowStartupLocation.CenterOwner };
        save.Click += (_, _) => { if (!string.IsNullOrWhiteSpace(name.Text) && !string.IsNullOrWhiteSpace(prompt.Text)) window.DialogResult = true; };
        Ui.Attach(window);
        return window.ShowDialog() == true ? new() { Id = initial.Id, Name = name.Text.Trim(), Prompt = prompt.Text.Trim(), Kind = (AnalysisKind)kinds.SelectedIndex } : null;
    }
}
