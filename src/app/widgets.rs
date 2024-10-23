use egui::{Align, Align2, Area, Frame, Key, Layout, Order, Response, Ui, UiKind};



pub fn theme_popup<R>(
    parent_ui: &Ui,
    widget_response: &Response,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {

    let (mut pos, pivot) = (widget_response.rect.left_bottom(), Align2::LEFT_TOP);
    if let Some(transform) = parent_ui
        .ctx()
        .memory(|m| m.layer_transforms.get(&parent_ui.layer_id()).copied())
    {
        pos = transform * pos;
    }

    let frame = Frame::popup(parent_ui.style());
    let frame_margin = frame.total_margin();
    let inner_width = widget_response.rect.width() - frame_margin.sum().x;

    let response = Area::new("custom_popup".into())
        .kind(UiKind::Popup)
        .order(Order::Foreground)
        .fixed_pos(pos)
        .default_width(inner_width)
        .pivot(pivot)
        .show(parent_ui.ctx(), |ui| {
            frame
                .show(ui, |ui| {
                    ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                        ui.set_min_width(inner_width);
                        add_contents(ui)
                    })
                    .inner
                })
                .inner
        });

    if parent_ui.input(|i| i.key_pressed(Key::Escape)) {
        parent_ui.memory_mut(|mem| mem.close_popup());
    }
    Some(response.inner)
}