use bevy::{color::palettes::css, prelude::*};
use bevy_mod_openxr::{add_xr_plugins, session::OxrSession};
use bevy_mod_xr::{session::XrSessionCreated, types::XrPose};
use bevy_suis::{
    debug::SuisDebugGizmosPlugin, xr::SuisXrPlugin, CaptureContext, Field, InputHandler,
    SuisCorePlugin,
};
use openxr::ReferenceSpaceType;

fn main() -> AppExit {
    App::new()
        .add_plugins(add_xr_plugins(DefaultPlugins))
        .add_plugins((SuisCorePlugin, SuisXrPlugin, SuisDebugGizmosPlugin))
        .add_systems(Startup, setup)
        .add_systems(XrSessionCreated, make_spectator_cam_follow)
        .run()
}

#[derive(Component)]
struct Cam;

fn make_spectator_cam_follow(
    query: Query<Entity, With<Cam>>,
    mut cmds: Commands,
    session: Res<OxrSession>,
) {
    let space = session
        .create_reference_space(ReferenceSpaceType::VIEW, Transform::IDENTITY)
        .unwrap();
    cmds.entity(query.single()).insert(space.0);
}

fn setup(mut cmds: Commands) {
    cmds.spawn((
        InputHandler::new(capture_condition),
        Field::Sphere(0.2),
        SpatialBundle::from_transform(Transform::from_xyz(0.0, 1.5, -1.0)),
    ));
    cmds.spawn((Camera3dBundle::default(), Cam));
}

fn capture_condition(ctx: In<CaptureContext>, query: Query<&Field>) -> bool {
    let Ok(field) = query.get(ctx.this_handler) else {
        warn!("Handler Somehow doesn't have a field?!");
        return false;
    };
    ctx.closest_point
        .distance(ctx.input_method_location.translation)
        <= f32::EPSILON
}
