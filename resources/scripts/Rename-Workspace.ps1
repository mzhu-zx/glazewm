Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

function RenameWorkspaceCore([string]$text) {
    $names = @($text -split ':',2 | % { $_.Trim() })
    switch ($names.Length) {
        1 {
            glazewm-cli.exe command update-workspace-config --display-name $names[0]
        }
        2 {
            if ($names[1]) {
                glazewm-cli.exe command update-workspace-config --name $names[0] --display-name $names[1]
            }
            else {
                glazewm-cli.exe command update-workspace-config --name $names[0] --no-display-name
            }
        }
    }
}

# Form size inputs you already had
$formX = 200
$formY = 100

# ---- layout constants (tweak as you like) ----
$pad    = 12                 # outer padding
$gap    = 8                  # vertical spacing between controls
$btnW   = 75
$btnH   = 23
$minW   = 300                # optional: sensible minimums
$minH   = 150

# ---- create form ----
$form = New-Object System.Windows.Forms.Form
$form.Text = 'Rename Workspace'
$form.Size = New-Object System.Drawing.Size($formX, $formY)
$form.StartPosition = 'CenterScreen'
$form.Topmost = $true
$form.MinimumSize = [System.Drawing.Size]::new($minW, $minH)
# Create a larger font (Segoe UI, 12pt regular)
$form.Font = New-Object System.Drawing.Font('Segoe UI', 12, [System.Drawing.FontStyle]::Regular)

# ---- controls ----
$okButton = New-Object System.Windows.Forms.Button
$okButton.Size = New-Object System.Drawing.Size($btnW, $btnH)
$okButton.Text = 'OK'
$okButton.DialogResult = [System.Windows.Forms.DialogResult]::OK
$form.AcceptButton = $okButton
$form.Controls.Add($okButton)

$label = New-Object System.Windows.Forms.Label
$label.Text = 'Name for the current workspace:'
$form.Controls.Add($label)

$textBox = New-Object System.Windows.Forms.TextBox
$form.Controls.Add($textBox)

# ---- responsive layout function ----
$UpdateLayout = {
    # Use ClientSize so we don't include borders/titlebar
    $cw = $form.ClientSize.Width
    $ch = $form.ClientSize.Height

    # Row 1: label
    $label.Left = $pad
    $label.Top  = $pad
    $label.Width  = [Math]::Max(0, $cw - 2*$pad)
    $label.Height = 20

    # Row 2: textbox (full width minus padding)
    $textBox.Left = $pad
    $textBox.Top  = $label.Bottom + $gap
    $textBox.Width  = [Math]::Max(0, $cw - 2*$pad)
    $textBox.Height = 20

    # Bottom row: buttons at bottom-right
    $yButtons = $ch - $pad - $btnH

    # If Cancel exists, place Cancel on the right, OK to its left; else only OK on the right
    if ($form.Controls.ContainsKey($cancelButton.Name)) {
        $cancelButton.Location = New-Object System.Drawing.Point(($cw - $pad - $btnW), $yButtons)
        $okButton.Location     = New-Object System.Drawing.Point(($cancelButton.Left - $gap - $btnW), $yButtons)
    } else {
        $okButton.Location     = New-Object System.Drawing.Point(($cw - $pad - $btnW), $yButtons)
    }

    # Make sure content has at least some height; grow form if too small
    # (optional guard if someone shrinks extremely)
    $minContentH = $pad + $label.Height + $gap + $textBox.Height + $gap + $btnH + $pad
    if ($ch -lt $minContentH) {
        $form.ClientSize = [System.Drawing.Size]::new($cw, $minContentH)
    }
}

# Hook layout updates
$form.add_SizeChanged($UpdateLayout)
# First layout pass (after Size is set)
& $UpdateLayout

$form.Add_Shown({
    $form.Activate()
    $form.BringToFront()
    $form.Focus()
    $textBox.Select()
})

# Close the form when Esc is pressed
$form.Add_KeyDown({
    if ($_.KeyCode -eq [System.Windows.Forms.Keys]::Escape) {
        $form.Close()
    }
})

# Make sure the form receives key events before controls do
$form.KeyPreview = $true

$result = $form.ShowDialog()

if ($result -eq [System.Windows.Forms.DialogResult]::OK) {
    $x = $textBox.Text
    RenameWorkspaceCore $x
}
