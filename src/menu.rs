use crate::{
    create_terrain_layer_material, make_scene_file_filter,
    scene::{
        commands::{graph::AddNodeCommand, sound::AddSoundSourceCommand, PasteCommand},
        EditorScene, Selection, SaveFileSelectorFlag,
    },
    send_sync_message,
    settings::{Settings, SettingsWindow},
    GameEngine, Message,
    utils::path_fixer::PathFixer,
    configurator::Configurator,
    interaction::{InteractionMode, InteractionModeTrait},
};
use rg3d::gui::message::UiMessage;
use rg3d::gui::{UiNode, UserInterface, BuildContext};
use rg3d::{
    core::{
        algebra::{Matrix4, Vector2},
        pool::Handle,
        scope_profile,
    },
    gui::{
        file_browser::{FileSelectorBuilder, FileBrowserMode},
        menu::{MenuBuilder, MenuItemBuilder, MenuItemContent},
        message::{
            FileSelectorMessage, MenuItemMessage, MessageBoxMessage, MessageDirection,
            UiMessageData, WidgetMessage, WindowMessage, KeyCode, MenuMessage,
        },
        messagebox::{MessageBoxBuilder, MessageBoxButtons, MessageBoxResult},
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        Thickness,
    },
    scene::{
        base::BaseBuilder,
        camera::CameraBuilder,
        decal::DecalBuilder,
        light::{
            directional::DirectionalLightBuilder, point::PointLightBuilder, spot::SpotLightBuilder,
            BaseLightBuilder,
        },
        mesh::{
            surface::{Surface, SurfaceData},
            Mesh, MeshBuilder,
        },
        node::Node,
        particle_system::{
            emitter::base::BaseEmitterBuilder, emitter::sphere::SphereEmitterBuilder,
            ParticleSystemBuilder,
        },
        sprite::SpriteBuilder,
        terrain::{LayerDefinition, TerrainBuilder},
    },
    sound::source::{generic::GenericSourceBuilder, spatial::SpatialSourceBuilder},
};
use std::{
    sync::{mpsc::Sender, Arc, RwLock},
    path::PathBuf,
};

pub struct Menu {
    pub menu: Handle<UiNode>,
    new_scene: Handle<UiNode>,
    save: Handle<UiNode>,
    save_as: Handle<UiNode>,
    load: Handle<UiNode>,
    close_scene: Handle<UiNode>,
    undo: Handle<UiNode>,
    redo: Handle<UiNode>,
    copy: Handle<UiNode>,
    paste: Handle<UiNode>,
    create_pivot: Handle<UiNode>,
    create_cube: Handle<UiNode>,
    create_cone: Handle<UiNode>,
    create_sphere: Handle<UiNode>,
    create_cylinder: Handle<UiNode>,
    create_quad: Handle<UiNode>,
    create_decal: Handle<UiNode>,
    create_point_light: Handle<UiNode>,
    create_spot_light: Handle<UiNode>,
    create_directional_light: Handle<UiNode>,
    create_terrain: Handle<UiNode>,
    exit: Handle<UiNode>,
    message_sender: Sender<Message>,
    save_file_selector: Handle<UiNode>,
    load_file_selector: Handle<UiNode>,
    create_camera: Handle<UiNode>,
    create_sprite: Handle<UiNode>,
    create_particle_system: Handle<UiNode>,
    create_sound_source: Handle<UiNode>,
    create_spatial_sound_source: Handle<UiNode>,
    sidebar: Handle<UiNode>,
    world_outliner: Handle<UiNode>,
    asset_browser: Handle<UiNode>,
    open_settings: Handle<UiNode>,
    configure: Handle<UiNode>,
    light_panel: Handle<UiNode>,
    pub settings: SettingsWindow,
    // menu-related widgets/objects moved from Editor to Menu class.
    pub configurator: Configurator,
    pub path_fixer: PathFixer,
    pub restriction: MenuShortcutRestriction,
    pub switch: MenuButtonSwitch,
    configure_message: Handle<UiNode>,
    log_panel: Handle<UiNode>,
    create: Handle<UiNode>,
    edit: Handle<UiNode>,
    open_path_fixer: Handle<UiNode>,
    // message box container
    pub message_boxes: MessageBoxes,
}

pub struct MenuContext<'a, 'b> {
    pub engine: &'a mut GameEngine,
    pub editor_scene: Option<&'b mut EditorScene>,
    pub sidebar_window: Handle<UiNode>,
    pub world_outliner_window: Handle<UiNode>,
    pub asset_window: Handle<UiNode>,
    pub light_panel: Handle<UiNode>,
    pub log_panel: Handle<UiNode>,
    pub settings: &'b mut Settings,
    pub current_interaction_mode: Option<&'b mut InteractionMode>,
}

// Purpose: restricts editor interaction when menu shortcut is used.
pub struct MenuShortcutRestriction {
    pub active: bool,
    // menu nodes that trigger a shortcut restriction.
    triggers: std::collections::HashSet<Handle<UiNode>>, 
}

// Purpose: stores handles to certain menu buttons, which dependent on whether scene is active.
pub struct MenuButtonSwitch {
    pub on: bool,
    // menu buttons affected by switch
    buttons: std::collections::HashSet<Handle<UiNode>>,
}

pub struct MessageBoxes {
    new_scene: Handle<UiNode>,
    close_scene: Handle<UiNode>,
    load_scene: Handle<UiNode>,
    pub exit: Handle<UiNode>,
    pub validation: Handle<UiNode>,
}

struct MessageBoxParams<'a> {
    width: f32,
    height: f32,
    position: Vector2<f32>,
    title: &'a str,
    text: &'a str,
    buttons: MessageBoxButtons
}

fn switch_window_state(window: Handle<UiNode>, ui: &mut UserInterface, center: bool) {
    let current_state = ui.node(window).visibility();
    ui.send_message(if current_state {
        WindowMessage::close(window, MessageDirection::ToWidget)
    } else {
        WindowMessage::open(window, MessageDirection::ToWidget, center)
    })
}

fn make_save_file_selector(ctx: &mut BuildContext) -> Handle<UiNode> {
    FileSelectorBuilder::new(
        WindowBuilder::new(WidgetBuilder::new().with_width(300.0).with_height(400.0))
            .with_title(WindowTitle::Text("Save Scene As".into()))
            .open(false),
    )
    .with_mode(FileBrowserMode::Save {
        default_file_name: PathBuf::from("unnamed.rgs"),
    })
    .with_path("./")
    .with_filter(make_scene_file_filter())
    .build(ctx)
}

fn make_load_file_selector(ctx: &mut BuildContext) -> Handle<UiNode> {
    FileSelectorBuilder::new(
        WindowBuilder::new(WidgetBuilder::new().with_width(300.0).with_height(400.0))
            .open(false)
            .with_title(WindowTitle::Text("Select a Scene To Load".into())),
    )
    .with_mode(FileBrowserMode::Open)
    .with_filter(make_scene_file_filter())
    .build(ctx)
}

fn make_message_box(ctx: &mut BuildContext, params: MessageBoxParams) -> Handle<UiNode> {
    MessageBoxBuilder::new(
        WindowBuilder::new(
            WidgetBuilder::new().with_width(params.width).with_height(params.height).with_desired_position(params.position))
                .can_close(false)
                .can_minimize(false)
                .open(false)   
                .with_title(WindowTitle::Text(params.title.to_owned()))
        )
        .with_text(params.text)
        .with_buttons(params.buttons)
        .build(ctx)
}


impl Menu {
    pub fn new(
        engine: &mut GameEngine,
        message_sender: Sender<Message>,
        settings: &Settings,
    ) -> Self {
        let min_size = Vector2::new(120.0, 22.0);
        let new_scene;
        let save;
        let save_as;
        let close_scene;
        let load;
        let redo;
        let undo;
        let copy;
        let paste;
        let create_cube;
        let create_cone;
        let create_sphere;
        let create_cylinder;
        let create_quad;
        let create_point_light;
        let create_spot_light;
        let create_directional_light;
        let exit;
        let create_camera;
        let create_sprite;
        let create_decal;
        let create_particle_system;
        let create_terrain;
        let sidebar;
        let asset_browser;
        let world_outliner;
        let open_settings;
        let configure;
        let light_panel;
        let log_panel;
        let create_pivot;
        let create_sound_source;
        let create_spatial_sound_source;
        let open_path_fixer;
        let ctx = &mut engine.user_interface.build_ctx();
        let configure_message = MessageBoxBuilder::new(
            WindowBuilder::new(WidgetBuilder::new().with_width(250.0).with_height(150.0))
                .open(false)
                .with_title(WindowTitle::Text("Warning".to_owned())),
        )
        .with_text("Cannot reconfigure editor while scene is open! Close scene first and retry.")
        .with_buttons(MessageBoxButtons::Ok)
        .build(ctx);

        let create = MenuItemBuilder::new(WidgetBuilder::new().with_margin(Thickness::right(10.0)))
            .with_content(MenuItemContent::text_with_shortcut("Create", ""))
            .with_items(vec![
                {
                    create_pivot =
                        MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                            .with_content(MenuItemContent::text("Pivot"))
                            .build(ctx);
                    create_pivot
                },
                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                    .with_content(MenuItemContent::text("Mesh"))
                    .with_items(vec![
                        {
                            create_cube =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Cube"))
                                    .build(ctx);
                            create_cube
                        },
                        {
                            create_sphere =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Sphere"))
                                    .build(ctx);
                            create_sphere
                        },
                        {
                            create_cylinder =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Cylinder"))
                                    .build(ctx);
                            create_cylinder
                        },
                        {
                            create_cone =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Cone"))
                                    .build(ctx);
                            create_cone
                        },
                        {
                            create_quad =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Quad"))
                                    .build(ctx);
                            create_quad
                        },
                    ])
                    .build(ctx),
                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                    .with_content(MenuItemContent::text("Sound"))
                    .with_items(vec![
                        {
                            create_sound_source =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("2D Source"))
                                    .build(ctx);
                            create_sound_source
                        },
                        {
                            create_spatial_sound_source =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("3D Source"))
                                    .build(ctx);
                            create_spatial_sound_source
                        },
                    ])
                    .build(ctx),
                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                    .with_content(MenuItemContent::text("Light"))
                    .with_items(vec![
                        {
                            create_directional_light =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Directional Light"))
                                    .build(ctx);
                            create_directional_light
                        },
                        {
                            create_spot_light =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Spot Light"))
                                    .build(ctx);
                            create_spot_light
                        },
                        {
                            create_point_light =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Point Light"))
                                    .build(ctx);
                            create_point_light
                        },
                    ])
                    .build(ctx),
                {
                    create_camera =
                        MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                            .with_content(MenuItemContent::text("Camera"))
                            .build(ctx);
                    create_camera
                },
                {
                    create_sprite =
                        MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                            .with_content(MenuItemContent::text("Sprite"))
                            .build(ctx);
                    create_sprite
                },
                {
                    create_particle_system =
                        MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                            .with_content(MenuItemContent::text("Particle System"))
                            .build(ctx);
                    create_particle_system
                },
                {
                    create_terrain =
                        MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                            .with_content(MenuItemContent::text("Terrain"))
                            .build(ctx);
                    create_terrain
                },
                {
                    create_decal =
                        MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                            .with_content(MenuItemContent::text("Decal"))
                            .build(ctx);
                    create_decal
                },
            ])
            .build(ctx);

        let edit = MenuItemBuilder::new(WidgetBuilder::new().with_margin(Thickness::right(10.0)))
            .with_content(MenuItemContent::text_with_shortcut("Edit", ""))
            .with_items(vec![
                {
                    undo = MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                        .with_content(MenuItemContent::text_with_shortcut("Undo", "Ctrl+Z"))
                        .build(ctx);
                    undo
                },
                {
                    redo = MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                        .with_content(MenuItemContent::text_with_shortcut("Redo", "Ctrl+Y"))
                        .build(ctx);
                    redo
                },
                {
                    copy = MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                        .with_content(MenuItemContent::text_with_shortcut("Copy", "Ctrl+C"))
                        .build(ctx);
                    copy
                },
                {
                    paste = MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                        .with_content(MenuItemContent::text_with_shortcut("Paste", "Ctrl+V"))
                        .build(ctx);
                    paste
                },
            ])
            .build(ctx);

        let menu = MenuBuilder::new(WidgetBuilder::new().on_row(0))
            .with_items(vec![
                MenuItemBuilder::new(WidgetBuilder::new().with_margin(Thickness::right(10.0)))
                    .with_content(MenuItemContent::text("File"))
                    .with_items(vec![
                        {
                            new_scene =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text_with_shortcut(
                                        "New Scene",
                                        "Ctrl+N",
                                    ))
                                    .build(ctx);
                            new_scene
                        },
                        {
                            save =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text_with_shortcut(
                                        "Save Scene",
                                        "Ctrl+S",
                                    ))
                                    .build(ctx);
                            save
                        },
                        {
                            save_as =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text_with_shortcut(
                                        "Save Scene As...",
                                        "Ctrl+Shift+S",
                                    ))
                                    .build(ctx);
                            save_as
                        },
                        {
                            load =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text_with_shortcut(
                                        "Load Scene...",
                                        "Ctrl+L",
                                    ))
                                    .build(ctx);
                            load
                        },
                        {
                            close_scene =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text_with_shortcut(
                                        "Close Scene",
                                        "Ctrl+Q",
                                    ))
                                    .build(ctx);
                            close_scene
                        },
                        {
                            open_settings =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Settings..."))
                                    .build(ctx);
                            open_settings
                        },
                        {
                            configure =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Configure..."))
                                    .build(ctx);
                            configure
                        },
                        {
                            exit =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text_with_shortcut(
                                        "Exit", "Alt+F4",
                                    ))
                                    .build(ctx);
                            exit
                        },
                    ])
                    .build(ctx),
                edit,
                create,
                MenuItemBuilder::new(WidgetBuilder::new().with_margin(Thickness::right(10.0)))
                    .with_content(MenuItemContent::text_with_shortcut("View", ""))
                    .with_items(vec![
                        {
                            sidebar =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Sidebar"))
                                    .build(ctx);
                            sidebar
                        },
                        {
                            asset_browser =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Asset Browser"))
                                    .build(ctx);
                            asset_browser
                        },
                        {
                            world_outliner =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("World Outliner"))
                                    .build(ctx);
                            world_outliner
                        },
                        {
                            light_panel =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Light Panel"))
                                    .build(ctx);
                            light_panel
                        },
                        {
                            log_panel =
                                MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                    .with_content(MenuItemContent::text("Log Panel"))
                                    .build(ctx);
                            log_panel
                        },
                    ])
                    .build(ctx),
                MenuItemBuilder::new(WidgetBuilder::new().with_margin(Thickness::right(10.0)))
                    .with_content(MenuItemContent::text_with_shortcut("Utils", ""))
                    .with_items(vec![{
                        open_path_fixer =
                            MenuItemBuilder::new(WidgetBuilder::new().with_min_size(min_size))
                                .with_content(MenuItemContent::text("Path Fixer"))
                                .build(ctx);
                        open_path_fixer
                    }])
                    .build(ctx),
            ])
            .build(ctx);

        let save_file_selector = make_save_file_selector(ctx);

        let load_file_selector = make_load_file_selector(ctx);

        let message_boxes = MessageBoxes {
            new_scene : make_message_box(ctx, MessageBoxParams {
                buttons: MessageBoxButtons::YesNoCancel,
                width: 290.0,
                height: 120.0,
                position: Vector2::new(0.0, 0.0),
                title: "Are you sure?",
                text: " Unsaved changes will be lost. Save before starting new scene?"
            }),
            close_scene : make_message_box(ctx, MessageBoxParams {
                buttons: MessageBoxButtons::YesNoCancel,
                width: 310.0,
                height: 180.0,
                position: Vector2::new(0.0, 0.0),
                title: "Closing the current scene.",
                text: " Do you want to save? Unsaved changes will be lost."
            }),
            load_scene : make_message_box(ctx, MessageBoxParams {
                buttons: MessageBoxButtons::YesNoCancel,
                width: 380.0,
                height: 120.0,
                position: Vector2::new(0.0, -310.0),
                title: "Loading a new scene.",
                text: " Unsaved changes to current scene will be lost. Do you want to save?"
            }),
            exit : make_message_box(ctx, MessageBoxParams {
                buttons: MessageBoxButtons::YesNoCancel,
                width: 380.0,
                height: 120.0,
                position: Vector2::new(0.0, 0.0),
                title: "Closing editor.",
                text: " Do you want to save changes to the scene before exiting?"
            }),
            validation : make_message_box(ctx, MessageBoxParams {
                buttons: MessageBoxButtons::Ok,
                width: 320.0,
                height: 240.0,
                position: Vector2::new(0.0, 200.0),
                title: "Error!",
                text: "
                        Saving failed! /n
                        Check the error log for more information.
                    "
            }),
        };

        let path_fixer = PathFixer::new(ctx);

        let settings_window = SettingsWindow::new(engine, message_sender.clone(), settings);

        let configurator = Configurator::new(
            message_sender.clone(),
            &mut engine.user_interface.build_ctx(),
        );
        engine
            .user_interface
            .send_message(WindowMessage::open_modal(
                configurator.window,
                MessageDirection::ToWidget,
                true,
            ));

        let mut restriction = MenuShortcutRestriction{active: false, triggers: std::collections::HashSet::new()};

        restriction.triggers.insert(message_boxes.new_scene);
        restriction.triggers.insert(message_boxes.close_scene);
        restriction.triggers.insert(message_boxes.load_scene);
        restriction.triggers.insert(message_boxes.exit);
        restriction.triggers.insert(message_boxes.validation);
        restriction.triggers.insert(load_file_selector);
        restriction.triggers.insert(save_file_selector);
        restriction.triggers.insert(path_fixer.window);
        restriction.triggers.insert(configurator.window);
        restriction.triggers.insert(settings_window.window);

        let mut switch = MenuButtonSwitch { on: false, buttons: std::collections::HashSet::new() };
        
        switch.buttons.insert(save);
        switch.buttons.insert(save_as);
        switch.buttons.insert(close_scene);
        switch.buttons.insert(create);
        switch.buttons.insert(edit);

        Self {
            menu,
            new_scene,
            save,
            save_as,
            close_scene,
            load,
            undo,
            redo,
            create_cube,
            create_cone,
            create_sphere,
            create_cylinder,
            create_quad,
            create_point_light,
            create_spot_light,
            create_directional_light,
            exit,
            settings: settings_window,
            restriction: restriction,
            switch: switch,
            message_sender,
            save_file_selector,
            load_file_selector,
            create_camera,
            create_sprite,
            create_particle_system,
            sidebar,
            world_outliner,
            asset_browser,
            open_settings,
            configure,
            configure_message,
            light_panel,
            copy,
            paste,
            log_panel,
            create_pivot,
            create_terrain,
            create_sound_source,
            create_spatial_sound_source,
            create,
            edit,
            open_path_fixer,
            path_fixer,
            configurator,
            create_decal,
            message_boxes,
        }
    }

    pub fn open_save_file_selector(&self, ui: &mut UserInterface) {
        ui.send_message(WindowMessage::open_modal(
            self.save_file_selector,
            MessageDirection::ToWidget,
            true,
        ));
        ui.send_message(FileSelectorMessage::path(
            self.save_file_selector,
            MessageDirection::ToWidget,
            std::env::current_dir().unwrap(),
        ));
    }

    pub fn open_load_file_selector(&self, ui: &mut UserInterface) {
        ui.send_message(WindowMessage::open_modal(
            self.load_file_selector,
            MessageDirection::ToWidget,
            true,
        ));
        ui.send_message(FileSelectorMessage::path(
            self.load_file_selector,
            MessageDirection::ToWidget,
            std::env::current_dir().unwrap(),
        ));
    }

    pub fn sync_to_model(&mut self, editor_scene: Option<&EditorScene>, ui: &mut UserInterface) {
        scope_profile!();
        self.switch.on = editor_scene.is_some();
        for &widget in self.switch.buttons.iter() {
            send_sync_message(
                ui,
                WidgetMessage::enabled(widget, MessageDirection::ToWidget, editor_scene.is_some()),
            );
        }
    }

    fn open_new_scene_message_box(&mut self, ui: &mut UserInterface) {
        ui
            .send_message(WindowMessage::open_modal(
                self.message_boxes.new_scene, // create new window object to open.
                MessageDirection::ToWidget,
                true,
        ));
    }

    fn open_close_scene_message_box(&mut self, ui: &mut UserInterface) {
        ui
            .send_message(WindowMessage::open_modal(
                self.message_boxes.close_scene, // create new window object to open.
                MessageDirection::ToWidget,
                true,
        ));
    }

    pub fn handle_ui_message(&mut self, message: &UiMessage, ctx: MenuContext) {
        scope_profile!();

        // editor scene param is option, so settings are still handled, when no scene is active.
        if let Some(scene) = ctx.editor_scene.as_ref() {
            self.settings
                .handle_message(message, Some(scene), ctx.engine, ctx.settings);
        } else {
            self.settings
                .handle_message(message, None, ctx.engine, ctx.settings);
        }

        self.configurator.handle_ui_message(message, ctx.engine);

        self.path_fixer
            .handle_ui_message(message, &mut ctx.engine.user_interface);

        match message.data() {
            UiMessageData::Window(WindowMessage::OpenModal {center} ) => {
                if self.restriction.triggers.contains(&message.destination()) {
                    self.restriction.active = true;
                    if let Some(scene) = ctx.editor_scene {
                        scene.camera_controller.freeze();
                        if let Some(mode) = ctx.current_interaction_mode {
                            mode.freeze(scene, ctx.engine);
                        }
                    }
                }
            }
            UiMessageData::Window(WindowMessage::Open {center} ) => {
                if self.restriction.triggers.contains(&message.destination()) {
                    self.restriction.active = true;
                    if let Some(scene) = ctx.editor_scene {
                        scene.camera_controller.freeze();
                        if let Some(mode) = ctx.current_interaction_mode {
                            mode.freeze(scene, ctx.engine);
                        }
                    }
                }
            }
            UiMessageData::Window(WindowMessage::Close) => {
                if self.restriction.triggers.contains(&message.destination()) {
                    self.restriction.active = false;
                    if let Some(scene) = ctx.editor_scene {
                        scene.camera_controller.unfreeze();
                        if let Some(mode) = ctx.current_interaction_mode {
                            mode.unfreeze();
                        }
                    }
                }
            }
            UiMessageData::FileSelector(FileSelectorMessage::Commit(path)) => {
                if message.destination() == self.save_file_selector {
                    if let Some(editor_scene) = ctx.editor_scene.as_ref() {
                        if let Some(flag) = editor_scene.selector_flag.as_ref() {
                            match flag {
                                SaveFileSelectorFlag::NewScene => {
                                    self.message_sender
                                        .send(Message::SaveScene(path.to_owned()))
                                        . unwrap();
                                    self.message_sender
                                        .send(Message::NewScene)
                                        .unwrap();
                                }
                                SaveFileSelectorFlag::ClosingScene => {
                                    self.message_sender
                                        .send(Message::SaveScene(path.to_owned()))
                                        . unwrap();
                                    self.message_sender
                                        .send(Message::CloseScene)
                                        .unwrap();
                                }
                                SaveFileSelectorFlag::LoadingScene => {
                                    self.message_sender
                                        .send(Message::SaveScene(path.to_owned()))
                                        .unwrap();
                                    self.message_sender
                                        .send(Message::LoadScene(editor_scene.next_path.as_ref().unwrap().clone()))
                                        .unwrap();
                                }
                                SaveFileSelectorFlag::Exiting => {
                                    self.message_sender
                                        .send(Message::SaveScene(path.clone()))
                                        .unwrap();
                                    self.message_sender
                                        .send(Message::Exit { force: true })
                                        .unwrap();
                                }
                            }
                        } else {
                            // just save scene
                            self.message_sender
                            .send(Message::SaveScene(path.to_owned()))
                            .unwrap();
                        }
                         // just save
                    } else {
                        self.message_sender
                            .send(Message::SaveScene(path.to_owned()))
                            .unwrap();
                    }
                } else if message.destination() == self.load_file_selector {
                    // active scene
                    if let Some(editor_scene) = ctx.editor_scene {
                        // scene active and save path determined.
                        // store load_path in editor scene based buffer.
                        editor_scene.next_path = Some(path.to_owned());          

                        ctx.engine
                            .user_interface
                            .send_message(WindowMessage::open_modal(
                                self.message_boxes.load_scene,
                                MessageDirection::ToWidget,
                                true,
                        ));
                    } else {
                        // just load scene
                        self.message_sender
                        .send(Message::LoadScene(path.to_owned()))
                        .unwrap();
                    }
                }
            }
            UiMessageData::MenuItem(MenuItemMessage::Click) => {
                if message.destination() == self.create_cube {
                    let mut mesh = Mesh::default();
                    mesh.set_name("Cube");
                    mesh.add_surface(Surface::new(Arc::new(RwLock::new(SurfaceData::make_cube(
                        Matrix4::identity(),
                    )))));
                    let node = Node::Mesh(mesh);
                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_spot_light {
                    let node = SpotLightBuilder::new(BaseLightBuilder::new(
                        BaseBuilder::new().with_name("SpotLight"),
                    ))
                    .with_distance(10.0)
                    .with_hotspot_cone_angle(45.0f32.to_radians())
                    .with_falloff_angle_delta(2.0f32.to_radians())
                    .build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_pivot {
                    let node = BaseBuilder::new().with_name("Pivot").build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_point_light {
                    let node = PointLightBuilder::new(BaseLightBuilder::new(
                        BaseBuilder::new().with_name("PointLight"),
                    ))
                    .with_radius(10.0)
                    .build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_directional_light {
                    let node = DirectionalLightBuilder::new(BaseLightBuilder::new(
                        BaseBuilder::new().with_name("DirectionalLight"),
                    ))
                    .build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_cone {
                    let mesh = MeshBuilder::new(BaseBuilder::new().with_name("Cone"))
                        .with_surfaces(vec![Surface::new(Arc::new(RwLock::new(
                            SurfaceData::make_cone(16, 0.5, 1.0, &Matrix4::identity()),
                        )))])
                        .build_node();
                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(mesh)))
                        .unwrap();
                } else if message.destination() == self.create_cylinder {
                    let mesh = MeshBuilder::new(BaseBuilder::new().with_name("Cylinder"))
                        .with_surfaces(vec![Surface::new(Arc::new(RwLock::new(
                            SurfaceData::make_cylinder(16, 0.5, 1.0, true, &Matrix4::identity()),
                        )))])
                        .build_node();
                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(mesh)))
                        .unwrap();
                } else if message.destination() == self.create_sphere {
                    let mesh = MeshBuilder::new(BaseBuilder::new().with_name("Sphere"))
                        .with_surfaces(vec![Surface::new(Arc::new(RwLock::new(
                            SurfaceData::make_sphere(16, 16, 0.5, &Matrix4::identity()),
                        )))])
                        .build_node();
                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(mesh)))
                        .unwrap();
                } else if message.destination() == self.create_quad {
                    let mesh = MeshBuilder::new(BaseBuilder::new().with_name("Quad"))
                        .with_surfaces(vec![Surface::new(Arc::new(RwLock::new(
                            SurfaceData::make_quad(&Matrix4::identity()),
                        )))])
                        .build_node();
                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(mesh)))
                        .unwrap();
                } else if message.destination() == self.create_camera {
                    let node = CameraBuilder::new(BaseBuilder::new().with_name("Camera"))
                        .enabled(false)
                        .build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_sprite {
                    let node =
                        SpriteBuilder::new(BaseBuilder::new().with_name("Sprite")).build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_sound_source {
                    let source = GenericSourceBuilder::new()
                        .with_name("2D Source")
                        .build_source()
                        .unwrap();

                    self.message_sender
                        .send(Message::do_scene_command(AddSoundSourceCommand::new(
                            source,
                        )))
                        .unwrap();
                } else if message.destination() == self.create_spatial_sound_source {
                    let source = SpatialSourceBuilder::new(
                        GenericSourceBuilder::new()
                            .with_name("3D Source")
                            .build()
                            .unwrap(),
                    )
                    .build_source();

                    self.message_sender
                        .send(Message::do_scene_command(AddSoundSourceCommand::new(
                            source,
                        )))
                        .unwrap();
                } else if message.destination() == self.create_particle_system {
                    let node =
                        ParticleSystemBuilder::new(BaseBuilder::new().with_name("ParticleSystem"))
                            .with_emitters(vec![SphereEmitterBuilder::new(
                                BaseEmitterBuilder::new()
                                    .with_max_particles(100)
                                    .resurrect_particles(true),
                            )
                            .with_radius(1.0)
                            .build()])
                            .build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_terrain {
                    let node = TerrainBuilder::new(BaseBuilder::new().with_name("Terrain"))
                        .with_layers(vec![LayerDefinition {
                            material: create_terrain_layer_material(),
                            mask_property_name: "maskTexture".to_owned(),
                        }])
                        .with_height_map_resolution(4.0)
                        .build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.create_decal {
                    let node =
                        DecalBuilder::new(BaseBuilder::new().with_name("Decal")).build_node();

                    self.message_sender
                        .send(Message::do_scene_command(AddNodeCommand::new(node)))
                        .unwrap();
                } else if message.destination() == self.save {
                    if let Some(scene_path) =
                        ctx.editor_scene.as_ref().map(|s| s.path.as_ref()).flatten()
                    {
                        self.message_sender
                            .send(Message::SaveScene(scene_path.clone()))
                            .unwrap();
                    } else {
                        self.open_save_file_selector(&mut ctx.engine.user_interface);
                    }    
                } else if message.destination() == self.save_as {
                    self.open_save_file_selector(&mut ctx.engine.user_interface);
                } else if message.destination() == self.load {
                    self.open_load_file_selector(&mut ctx.engine.user_interface);
                } else if message.destination() == self.close_scene {
                    if let Some(editor_scene) = ctx.editor_scene {
                        self.open_close_scene_message_box(&mut ctx.engine.user_interface);
                    } else {
                        self.message_sender
                            .send(Message::CloseScene).unwrap();
                    }
                } else if message.destination() == self.copy {
                    if let Some(editor_scene) = ctx.editor_scene {
                        if let Selection::Graph(selection) = &editor_scene.selection {
                            editor_scene.clipboard.fill_from_selection(
                                selection,
                                editor_scene.scene,
                                &editor_scene.physics,
                                ctx.engine,
                            );
                        }
                    }
                } else if message.destination() == self.paste {
                    if let Some(editor_scene) = ctx.editor_scene {
                        if !editor_scene.clipboard.is_empty() {
                            self.message_sender
                                .send(Message::do_scene_command(PasteCommand::new()))
                                .unwrap();
                        }
                    }
                } else if message.destination() == self.undo {
                    self.message_sender.send(Message::UndoSceneCommand).unwrap();
                } else if message.destination() == self.redo {
                    self.message_sender.send(Message::RedoSceneCommand).unwrap();
                } else if message.destination() == self.exit {
                    self.message_sender
                        .send(Message::Exit { force: false })
                        .unwrap();
                } else if message.destination() == self.new_scene {
                    if let Some(editor_scene) = ctx.editor_scene.as_ref() {
                        self.open_new_scene_message_box(&mut ctx.engine.user_interface);
                    } else {
                        self.message_sender
                            .send(Message::NewScene).unwrap();
                    }
                } else if message.destination() == self.asset_browser {
                    switch_window_state(ctx.asset_window, &mut ctx.engine.user_interface, false);
                } else if message.destination() == self.light_panel {
                    switch_window_state(ctx.light_panel, &mut ctx.engine.user_interface, true);
                } else if message.destination() == self.world_outliner {
                    switch_window_state(
                        ctx.world_outliner_window,
                        &mut ctx.engine.user_interface,
                        false,
                    );
                } else if message.destination() == self.sidebar {
                    switch_window_state(ctx.sidebar_window, &mut ctx.engine.user_interface, false);
                } else if message.destination() == self.log_panel {
                    switch_window_state(ctx.log_panel, &mut ctx.engine.user_interface, false);
                } else if message.destination() == self.open_settings {
                    self.settings
                        .open(&ctx.engine.user_interface, ctx.settings, None);
                } else if message.destination() == self.open_path_fixer {
                    ctx.engine
                        .user_interface
                        .send_message(WindowMessage::open_modal(
                            self.path_fixer.window,
                            MessageDirection::ToWidget,
                            true,
                        ));
                } else if message.destination() == self.configure {
                    if ctx.editor_scene.is_none() {
                        ctx.engine
                            .user_interface
                            .send_message(WindowMessage::open_modal(
                                self.configurator.window,
                                MessageDirection::ToWidget,
                                true,
                            ));
                    } else {
                        ctx.engine
                            .user_interface
                            .send_message(MessageBoxMessage::open(
                                self.configure_message,
                                MessageDirection::ToWidget,
                                None,
                                None,
                            ));
                    }
                }
            }
            UiMessageData::Widget(WidgetMessage::KeyDown(key)) => {
                if !self.restriction.active {
                    match *key {
                        KeyCode::L => {
                            if ctx.engine.user_interface.keyboard_modifiers().control  {
                                self.open_load_file_selector(&mut ctx.engine.user_interface);
                            }
                        }
                        KeyCode::N => {
                            if ctx.engine.user_interface.keyboard_modifiers().control {
                                if let Some(editor_scene) = ctx.editor_scene {
                                    self.open_new_scene_message_box(&mut ctx.engine.user_interface);
                                } else {
                                    self.message_sender
                                        .send(Message::NewScene).unwrap();
                                }
                            }
                        }
                        KeyCode::Q => {
                            if ctx.engine.user_interface.keyboard_modifiers().control {
                                if let Some(editor_scene) = ctx.editor_scene {
                                    self.open_close_scene_message_box(&mut ctx.engine.user_interface);
                                } else {
                                    self.message_sender
                                        .send(Message::CloseScene).unwrap();
                                }
                            }
                        }
                        KeyCode::S => {
                            if ctx.engine.user_interface.keyboard_modifiers().control &&  ctx.engine.user_interface.keyboard_modifiers().shift {   
                                if let Some(editor_scene) = ctx.editor_scene {
                                    self.open_save_file_selector(&mut ctx.engine.user_interface);
                                }
                            } else if ctx.engine.user_interface.keyboard_modifiers().control {
                                if let Some(editor_scene) = ctx.editor_scene {
                                    if let Some(scene_path) = editor_scene.path.as_ref()
                                    {
                                        self.message_sender
                                            .send(Message::SaveScene(scene_path.clone()))
                                            .unwrap();
                                    } else {
                                        self.open_save_file_selector(&mut ctx.engine.user_interface);
                                    }     
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
            UiMessageData::MessageBox(MessageBoxMessage::Close(result)) => {
                if message.destination() == self.message_boxes.exit {
                    match result {
                        MessageBoxResult::No => {
                            self.message_sender
                                .send(Message::Exit { force: true })
                                .unwrap();
                        }
                        MessageBoxResult::Yes => {
                            if let Some(scene) = ctx.editor_scene {
                                if let Some(path) = scene.path.as_ref() {
                                    self.message_sender
                                        .send(Message::SaveScene(path.clone()))
                                        .unwrap();
                                    self.message_sender
                                        .send(Message::Exit { force: true })
                                        .unwrap();
                                } else {
                                    scene.selector_flag = Some(SaveFileSelectorFlag::Exiting);
                                    // Scene wasn't saved yet, open Save As dialog.
                                    self.open_save_file_selector(&mut ctx.engine.user_interface);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                else if message.destination() == self.message_boxes.new_scene {
                    match result {
                        MessageBoxResult::No => {
                            if let Some(scene) = ctx.editor_scene.as_ref() {
                                self.message_sender
                                    .send(Message::NewScene)
                                    .unwrap();
                            }
                        }
                        MessageBoxResult::Yes => {
                            if let Some(scene) = ctx.editor_scene {
                                if let Some(path) = scene.path.as_ref() {
                                    self.message_sender
                                        .send(Message::SaveScene(path.clone()))
                                        . unwrap();
                                    self.message_sender
                                        .send(Message::NewScene)
                                        .unwrap();
                                } else {
                                    scene.selector_flag = Some(SaveFileSelectorFlag::NewScene);
                                    // Scene wasn't saved yet, open Save As dialog.
                                    self.open_save_file_selector(&mut ctx.engine.user_interface);
                                }
                            }
                        }
                        _ => {}    
                    }
                } else if message.destination() == self.message_boxes.close_scene {
                    match result {
                        MessageBoxResult::No => {
                            if let Some(scene) = ctx.editor_scene.as_ref() {
                                self.message_sender
                                .send(Message::CloseScene)
                                .unwrap();
                            }
                        }
                        MessageBoxResult::Yes => {
                            if let Some(scene) = ctx.editor_scene { // might not be needed since message box means a scene is active
                                if let Some(path) = scene.path.as_ref() {
                                    self.message_sender
                                        .send(Message::SaveScene(path.clone()))
                                        .unwrap();
                                    self.message_sender
                                        .send(Message::CloseScene)
                                        .unwrap();
                                    scene.selector_flag = None;
                                } else {
                                    // Scene wasn't saved yet, open Save As dialog.
                                    scene.selector_flag = Some(SaveFileSelectorFlag::ClosingScene);
                                    self.open_save_file_selector(&mut ctx.engine.user_interface);
                                }
                            }
                        }
                        _ => {}
                    }
                } else if message.destination() == self.message_boxes.load_scene {
                    match result {
                        // no saving before loading scene
                        MessageBoxResult::No => {
                            if let Some(scene) = ctx.editor_scene.as_ref() {
                                self.message_sender
                                // load_path variable updated in load_file_Selector commit event
                                .send(Message::LoadScene(scene.next_path.as_ref().unwrap().to_owned()))
                                .unwrap();
                            }
                        }
                        // saving before loading scene
                        MessageBoxResult::Yes => {
                            if let Some(scene) = ctx.editor_scene {
                                if let Some(path) = scene.path.as_ref() {
                                    self.message_sender
                                        .send(Message::SaveScene(path.to_owned()))
                                        .unwrap();
                                    self.message_sender
                                        .send(Message::LoadScene(scene.next_path.as_ref().unwrap().to_owned()))
                                        .unwrap();
                                } else {
                                    // Scene wasn't saved yet, open Save As dialog.
                                    scene.selector_flag = Some(SaveFileSelectorFlag::LoadingScene);
                                    self.open_save_file_selector(&mut ctx.engine.user_interface);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => (),
        }
    }
}
