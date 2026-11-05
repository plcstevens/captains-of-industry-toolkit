using System;
using Mafi;
using Mafi.Localization;
using Mafi.Unity.InputControl;
using Mafi.Unity.Ui;
using Mafi.Unity.Ui.Hud;
using Mafi.Unity.UiStatic.Toolbar;
using Mafi.Unity.UiToolkit.Library;
using QoSCore;

namespace QoSWorldMap;

/// <summary>
/// Toolbar controller for the world-mines window — the
/// CoI_PriorityManager shape exactly: a ControllerConfig.Window
/// toolbar item whose Activate() refreshes and opens the window,
/// and whose window-close deactivates the controller again.
///
/// Live-check seam 4 (toolbar slot): the game's own world-map
/// button sits at order 520. ToolbarOrder below is 1500 — the slot
/// CoI_PriorityManager ships with, i.e. proven free in a real mod —
/// but whether it collides with any vanilla 0.8.8 button is a
/// first-hour live check; this constant is the only place to move.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class WorldMinesController : IToolbarItemController {

	/// <summary>Toolbar ordering slot (live-check seam 4 — see class docs).</summary>
	public const float ToolbarOrder = 1500f;

	private readonly WorldMinesWindow m_window;
	private readonly UiContext m_context;
	private bool m_isActive;

	public WorldMinesController(WorldMinesWindow window, ToolbarHud toolbar, UiContext context) {
		m_window = window;
		m_context = context;
		toolbar.AddMainMenuButton(
			new LocStrFormatted("World mines"),
			this,
			// Icon: PriorityManager's proven General icon path. A
			// dedicated map/mine glyph is a cosmetic live check —
			// guessing an unverified asset path risks a blank slot.
			"Assets/Unity/UserInterface/General/Priority.svg",
			ToolbarOrder);
		// No shortcut argument: the parameter is Option<KbHandle>
		// (a value type — null does not convert), its declared
		// default is "no shortcut", and picking a key is part of
		// live-check seam 4 anyway.
		window.OnCloseStart += OnWindowClose;
		VisibilityChanged?.Invoke(this);
		QoSLog.Info("WorldMap", "toolbar button registered at order " + ToolbarOrder);
	}

	public ControllerConfig Config { get { return ControllerConfig.Window; } }

	// Toolbar visibility means "available to activate", not "window
	// currently open" (PriorityManager's note, kept verbatim).
	public bool IsVisible { get { return true; } }

	public bool DeactivateShortcutsIfNotVisible { get { return false; } }

	public event Action<IToolbarItemController> VisibilityChanged;

	public void Activate() {
		if (m_isActive) {
			return;
		}
		m_isActive = true;
		m_window.Refresh();
		m_window.Open(m_context.UiRoot);
	}

	public void Deactivate() {
		if (!m_isActive) {
			return;
		}
		m_isActive = false;
		m_window.Close();
	}

	public bool InputUpdate() {
		m_window.PollForChanges();
		return m_window.InputUpdate();
	}

	private void OnWindowClose(Window _) {
		if (m_isActive) {
			m_context.InputMgr.DeactivateController(this);
		}
	}
}
