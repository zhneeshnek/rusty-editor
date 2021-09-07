use crate::{
    gui::{BuildContext, EditorUiNode, Ui, UiMessage, UiNode},
    make_relative_path,
    preview::PreviewPanel,
    scene::commands::{material::SetMaterialPropertyValueCommand, SceneCommand},
    send_sync_message, GameEngine, Message,
};
use rg3d::{
    core::{
        algebra::{Matrix4, Vector2, Vector3, Vector4},
        pool::Handle,
        BiDirHashMap,
    },
    gui::{
        border::BorderBuilder,
        check_box::CheckBoxBuilder,
        color::ColorFieldBuilder,
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        list_view::ListViewBuilder,
        message::{
            CheckBoxMessage, ColorFieldMessage, ImageMessage, ListViewMessage, MessageDirection,
            NumericUpDownMessage, UiMessageData, Vec2EditorMessage, Vec3EditorMessage,
            Vec4EditorMessage, WidgetMessage,
        },
        numeric::NumericUpDownBuilder,
        scroll_viewer::ScrollViewerBuilder,
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        vec::{vec2::Vec2EditorBuilder, vec3::Vec3EditorBuilder, vec4::Vec4EditorBuilder},
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        Thickness, VerticalAlignment,
    },
    material::{Material, PropertyValue},
    scene::{
        base::BaseBuilder,
        mesh::{
            surface::{SurfaceBuilder, SurfaceData},
            MeshBuilder,
        },
    },
    utils::into_gui_texture,
};
use std::sync::{mpsc::Sender, Arc, Mutex, RwLock};

pub struct MaterialEditor {
    pub window: Handle<UiNode>,
    properties_panel: Handle<UiNode>,
    properties: BiDirHashMap<String, Handle<UiNode>>,
    preview: PreviewPanel,
    material: Option<Arc<Mutex<Material>>>,
}

fn create_item_container(
    ctx: &mut BuildContext,
    name: &str,
    item: Handle<UiNode>,
) -> Handle<UiNode> {
    ctx[item].set_column(1);

    GridBuilder::new(
        WidgetBuilder::new()
            .with_margin(Thickness::uniform(1.0))
            .with_child(
                TextBuilder::new(WidgetBuilder::new())
                    .with_text(name)
                    .with_vertical_text_alignment(VerticalAlignment::Center)
                    .build(ctx),
            )
            .with_child(item),
    )
    .add_row(Row::strict(24.0))
    .add_column(Column::strict(150.0))
    .add_column(Column::stretch())
    .build(ctx)
}

fn create_array_view<T, B>(
    ctx: &mut BuildContext,
    value: &[T],
    mut item_builder: B,
) -> Handle<UiNode>
where
    T: Clone,
    B: FnMut(&mut BuildContext, T) -> Handle<UiNode>,
{
    ListViewBuilder::new(WidgetBuilder::new().with_height(24.0))
        .with_items(value.iter().map(|v| item_builder(ctx, v.clone())).collect())
        .build(ctx)
}

fn create_array_of_array_view<'a, T, B, I>(
    ctx: &mut BuildContext,
    value: I,
    mut item_builder: B,
) -> Handle<UiNode>
where
    T: 'a + Clone,
    B: FnMut(&mut BuildContext, T) -> Handle<UiNode>,
    I: Iterator<Item = &'a [T]>,
{
    ListViewBuilder::new(WidgetBuilder::new().with_height(24.0))
        .with_items(
            value
                .map(|v| create_array_view(ctx, v, &mut item_builder))
                .collect(),
        )
        .build(ctx)
}

fn create_float_view(ctx: &mut BuildContext, value: f32) -> Handle<UiNode> {
    NumericUpDownBuilder::new(WidgetBuilder::new().with_height(24.0))
        .with_value(value)
        .build(ctx)
}

fn create_int_view(ctx: &mut BuildContext, value: i32) -> Handle<UiNode> {
    NumericUpDownBuilder::new(WidgetBuilder::new().with_height(24.0))
        .with_value(value as f32)
        .with_precision(0)
        .with_max_value(i32::MAX as f32)
        .with_min_value(-i32::MAX as f32)
        .build(ctx)
}

fn create_uint_view(ctx: &mut BuildContext, value: u32) -> Handle<UiNode> {
    NumericUpDownBuilder::new(WidgetBuilder::new().with_height(24.0))
        .with_value(value as f32)
        .with_precision(0)
        .with_max_value(u32::MAX as f32)
        .with_min_value(0.0)
        .build(ctx)
}

fn create_vec2_view(ctx: &mut BuildContext, value: Vector2<f32>) -> Handle<UiNode> {
    Vec2EditorBuilder::new(WidgetBuilder::new().with_height(24.0))
        .with_value(value)
        .build(ctx)
}

fn create_vec3_view(ctx: &mut BuildContext, value: Vector3<f32>) -> Handle<UiNode> {
    Vec3EditorBuilder::new(WidgetBuilder::new().with_height(24.0))
        .with_value(value)
        .build(ctx)
}

fn create_vec4_view(ctx: &mut BuildContext, value: Vector4<f32>) -> Handle<UiNode> {
    Vec4EditorBuilder::new(WidgetBuilder::new().with_height(24.0))
        .with_value(value)
        .build(ctx)
}

fn sync_array<T, B>(ui: &mut Ui, handle: Handle<UiNode>, array: &[T], mut item_builder: B)
where
    T: Clone,
    B: FnMut(&mut BuildContext, T) -> Handle<UiNode>,
{
    let ctx = &mut ui.build_ctx();

    let new_items = array.iter().map(|v| item_builder(ctx, v.clone())).collect();

    send_sync_message(
        ui,
        ListViewMessage::items(handle, MessageDirection::ToWidget, new_items),
    );
}

fn sync_array_of_arrays<'a, T, I, B>(
    ui: &mut Ui,
    handle: Handle<UiNode>,
    array: I,
    mut item_builder: B,
) where
    T: 'a + Clone,
    I: Iterator<Item = &'a [T]>,
    B: FnMut(&mut BuildContext, T) -> Handle<UiNode>,
{
    let ctx = &mut ui.build_ctx();

    let new_items = array
        .map(|v| create_array_view(ctx, v, &mut item_builder))
        .collect();

    send_sync_message(
        ui,
        ListViewMessage::items(handle, MessageDirection::ToWidget, new_items),
    );
}

impl MaterialEditor {
    pub fn new(engine: &mut GameEngine) -> Self {
        let mut preview = PreviewPanel::new(engine, 300, 400);

        let graph = &mut engine.scenes[preview.scene()].graph;
        let sphere = MeshBuilder::new(BaseBuilder::new())
            .with_surfaces(vec![SurfaceBuilder::new(Arc::new(RwLock::new(
                SurfaceData::make_sphere(30, 30, 1.0, &Matrix4::identity()),
            )))
            .build()])
            .build(graph);
        preview.set_model(sphere, engine);

        let ctx = &mut engine.user_interface.build_ctx();

        let panel;
        let properties_panel;
        let window = WindowBuilder::new(WidgetBuilder::new().with_width(300.0))
            .open(false)
            .with_title(WindowTitle::text("Material Editor"))
            .with_content(
                GridBuilder::new(
                    WidgetBuilder::new()
                        .with_child(
                            ScrollViewerBuilder::new(WidgetBuilder::new())
                                .with_content({
                                    properties_panel =
                                        StackPanelBuilder::new(WidgetBuilder::new()).build(ctx);
                                    properties_panel
                                })
                                .build(ctx),
                        )
                        .with_child({
                            panel = BorderBuilder::new(WidgetBuilder::new().on_row(1).on_column(0))
                                .build(ctx);
                            panel
                        }),
                )
                .add_row(Row::stretch())
                .add_row(Row::strict(300.0))
                .add_column(Column::stretch())
                .build(ctx),
            )
            .build(ctx);

        ctx.link(preview.root, panel);

        Self {
            window,
            preview,
            properties_panel,
            properties: Default::default(),
            material: None,
        }
    }

    pub fn set_material(
        &mut self,
        material: Option<Arc<Mutex<Material>>>,
        engine: &mut GameEngine,
    ) {
        self.material = material;

        if let Some(material) = self.material.clone() {
            engine.scenes[self.preview.scene()].graph[self.preview.model()]
                .as_mesh_mut()
                .surfaces_mut()
                .first_mut()
                .unwrap()
                .set_material(material);
        }

        self.sync_to_model(&mut engine.user_interface);
    }

    pub fn sync_to_model(&mut self, ui: &mut Ui) {
        if let Some(material) = self.material.as_ref() {
            let material = material.lock().unwrap();

            // Keep properties in sync.
            if material.properties().len() < self.properties.len() {
                for name in self
                    .properties
                    .forward_map()
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    if !material.properties().contains_key(&name) {
                        let item_to_delete = ui
                            .node(
                                self.properties
                                    .remove_by_key(&name)
                                    .expect("Desync has occurred!"),
                            )
                            .parent();

                        send_sync_message(
                            ui,
                            WidgetMessage::remove(item_to_delete, MessageDirection::ToWidget),
                        );
                    }
                }
            } else if material.properties().len() > self.properties.len() {
                for (name, property_value) in material.properties() {
                    if !self.properties.contains_key(name) {
                        let ctx = &mut ui.build_ctx();

                        let item = match property_value {
                            PropertyValue::Float(value) => create_float_view(ctx, *value),
                            PropertyValue::FloatArray(value) => {
                                create_array_view(ctx, value, create_float_view)
                            }
                            PropertyValue::Int(value) => create_int_view(ctx, *value),
                            PropertyValue::IntArray(value) => {
                                create_array_view(ctx, value, create_int_view)
                            }
                            PropertyValue::UInt(value) => create_uint_view(ctx, *value),
                            PropertyValue::UIntArray(value) => {
                                create_array_view(ctx, value, create_uint_view)
                            }
                            PropertyValue::Vector2(value) => create_vec2_view(ctx, *value),
                            PropertyValue::Vector2Array(value) => {
                                create_array_view(ctx, value, create_vec2_view)
                            }
                            PropertyValue::Vector3(value) => create_vec3_view(ctx, *value),
                            PropertyValue::Vector3Array(value) => {
                                create_array_view(ctx, value, create_vec3_view)
                            }
                            PropertyValue::Vector4(value) => create_vec4_view(ctx, *value),
                            PropertyValue::Vector4Array(value) => {
                                create_array_view(ctx, value, create_vec4_view)
                            }
                            PropertyValue::Matrix2(value) => {
                                create_array_view(ctx, value.data.as_slice(), create_float_view)
                            }
                            PropertyValue::Matrix2Array(value) => create_array_of_array_view(
                                ctx,
                                value.iter().map(|m| m.data.as_slice()),
                                create_float_view,
                            ),
                            PropertyValue::Matrix3(value) => {
                                create_array_view(ctx, value.data.as_slice(), create_float_view)
                            }
                            PropertyValue::Matrix3Array(value) => create_array_of_array_view(
                                ctx,
                                value.iter().map(|m| m.data.as_slice()),
                                create_float_view,
                            ),
                            PropertyValue::Matrix4(value) => {
                                create_array_view(ctx, value.data.as_slice(), create_float_view)
                            }
                            PropertyValue::Matrix4Array(value) => create_array_of_array_view(
                                ctx,
                                value.iter().map(|m| m.data.as_slice()),
                                create_float_view,
                            ),
                            PropertyValue::Bool(value) => {
                                CheckBoxBuilder::new(WidgetBuilder::new())
                                    .checked(Some(*value))
                                    .build(ctx)
                            }
                            PropertyValue::Color(value) => {
                                ColorFieldBuilder::new(WidgetBuilder::new())
                                    .with_color(*value)
                                    .build(ctx)
                            }
                            PropertyValue::Sampler { value, .. } => {
                                ImageBuilder::new(WidgetBuilder::new().with_allow_drop(true))
                                    .with_opt_texture(value.clone().map(|t| into_gui_texture(t)))
                                    .build(ctx)
                            }
                        };

                        self.properties.insert(name.to_owned(), item);

                        let container = create_item_container(ctx, name, item);

                        send_sync_message(
                            ui,
                            WidgetMessage::link(
                                container,
                                MessageDirection::ToWidget,
                                self.properties_panel,
                            ),
                        );
                    }
                }
            }

            // Sync values.
            for (name, property_value) in material.properties() {
                let item = *self
                    .properties
                    .value_of(name)
                    .expect("Desync has occurred.");

                match property_value {
                    PropertyValue::Float(value) => {
                        send_sync_message(
                            ui,
                            NumericUpDownMessage::value(item, MessageDirection::ToWidget, *value),
                        );
                    }
                    PropertyValue::FloatArray(value) => {
                        sync_array(ui, item, value, create_float_view)
                    }
                    PropertyValue::Int(value) => {
                        send_sync_message(
                            ui,
                            NumericUpDownMessage::value(
                                item,
                                MessageDirection::ToWidget,
                                *value as f32,
                            ),
                        );
                    }
                    PropertyValue::IntArray(value) => sync_array(ui, item, value, create_int_view),
                    PropertyValue::UInt(value) => {
                        send_sync_message(
                            ui,
                            NumericUpDownMessage::value(
                                item,
                                MessageDirection::ToWidget,
                                *value as f32,
                            ),
                        );
                    }
                    PropertyValue::UIntArray(value) => {
                        sync_array(ui, item, value, create_uint_view)
                    }
                    PropertyValue::Vector2(value) => send_sync_message(
                        ui,
                        Vec2EditorMessage::value(item, MessageDirection::ToWidget, *value),
                    ),
                    PropertyValue::Vector2Array(value) => {
                        sync_array(ui, item, value, create_vec2_view)
                    }
                    PropertyValue::Vector3(value) => send_sync_message(
                        ui,
                        Vec3EditorMessage::value(item, MessageDirection::ToWidget, *value),
                    ),
                    PropertyValue::Vector3Array(value) => {
                        sync_array(ui, item, value, create_vec3_view)
                    }
                    PropertyValue::Vector4(value) => send_sync_message(
                        ui,
                        Vec4EditorMessage::value(item, MessageDirection::ToWidget, *value),
                    ),
                    PropertyValue::Vector4Array(value) => {
                        sync_array(ui, item, value, create_vec4_view)
                    }
                    PropertyValue::Matrix2(value) => {
                        sync_array(ui, item, value.as_slice(), create_float_view)
                    }
                    PropertyValue::Matrix2Array(value) => sync_array_of_arrays(
                        ui,
                        item,
                        value.iter().map(|m| m.as_slice()),
                        create_float_view,
                    ),
                    PropertyValue::Matrix3(value) => {
                        sync_array(ui, item, value.as_slice(), create_float_view)
                    }
                    PropertyValue::Matrix3Array(value) => sync_array_of_arrays(
                        ui,
                        item,
                        value.iter().map(|m| m.as_slice()),
                        create_float_view,
                    ),
                    PropertyValue::Matrix4(value) => {
                        sync_array(ui, item, value.as_slice(), create_float_view)
                    }
                    PropertyValue::Matrix4Array(value) => sync_array_of_arrays(
                        ui,
                        item,
                        value.iter().map(|m| m.as_slice()),
                        create_float_view,
                    ),
                    PropertyValue::Bool(value) => {
                        send_sync_message(
                            ui,
                            CheckBoxMessage::checked(
                                item,
                                MessageDirection::ToWidget,
                                Some(*value),
                            ),
                        );
                    }
                    PropertyValue::Color(value) => {
                        send_sync_message(
                            ui,
                            ColorFieldMessage::color(item, MessageDirection::ToWidget, *value),
                        );
                    }
                    PropertyValue::Sampler { value, .. } => send_sync_message(
                        ui,
                        ImageMessage::texture(
                            item,
                            MessageDirection::ToWidget,
                            value.clone().map(|t| into_gui_texture(t)),
                        ),
                    ),
                }
            }
        } else {
            send_sync_message(
                ui,
                ListViewMessage::items(self.properties_panel, MessageDirection::ToWidget, vec![]),
            );
        }
    }

    pub fn handle_ui_message(
        &mut self,
        message: &UiMessage,
        engine: &mut GameEngine,
        sender: &Sender<Message>,
    ) {
        self.preview.handle_message(message, engine);

        if let Some(material) = self.material.as_ref() {
            if let Some(property_name) = self.properties.key_of(&message.destination()) {
                let property_value = match message.data() {
                    UiMessageData::NumericUpDown(NumericUpDownMessage::Value(value))
                        if message.direction() == MessageDirection::FromWidget =>
                    {
                        // NumericUpDown is used for Float, Int, UInt properties, so we have to check
                        // the actual property "type" to create suitable value from f32.
                        match material
                            .lock()
                            .unwrap()
                            .property_ref(property_name)
                            .unwrap()
                        {
                            PropertyValue::Float(_) => Some(PropertyValue::Float(*value)),
                            PropertyValue::Int(_) => Some(PropertyValue::Int(*value as i32)),
                            PropertyValue::UInt(_) => Some(PropertyValue::UInt(*value as u32)),
                            _ => None,
                        }
                    }
                    UiMessageData::Vec2Editor(Vec2EditorMessage::Value(value))
                        if message.direction() == MessageDirection::FromWidget =>
                    {
                        Some(PropertyValue::Vector2(*value))
                    }
                    UiMessageData::Vec3Editor(Vec3EditorMessage::Value(value))
                        if message.direction() == MessageDirection::FromWidget =>
                    {
                        Some(PropertyValue::Vector3(*value))
                    }
                    UiMessageData::Vec4Editor(Vec4EditorMessage::Value(value))
                        if message.direction() == MessageDirection::FromWidget =>
                    {
                        Some(PropertyValue::Vector4(*value))
                    }

                    UiMessageData::Widget(WidgetMessage::Drop(handle)) => {
                        if let UiNode::User(EditorUiNode::AssetItem(asset_item)) =
                            engine.user_interface.node(*handle)
                        {
                            let relative_path = make_relative_path(&asset_item.path);

                            let texture =
                                Some(engine.resource_manager.request_texture(relative_path, None));

                            engine.user_interface.send_message(ImageMessage::texture(
                                message.destination(),
                                MessageDirection::ToWidget,
                                texture.clone().map(|t| into_gui_texture(t)),
                            ));

                            Some(PropertyValue::Sampler {
                                value: texture,
                                fallback: Default::default(),
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(property_value) = property_value {
                    sender
                        .send(Message::DoSceneCommand(
                            SceneCommand::SetMaterialPropertyValue(
                                SetMaterialPropertyValueCommand::new(
                                    material.clone(),
                                    property_name.clone(),
                                    property_value,
                                ),
                            ),
                        ))
                        .unwrap();
                }
            }
        }
    }

    pub fn update(&mut self, engine: &mut GameEngine) {
        self.preview.update(engine)
    }
}