// Any copyright is dedicated to the Public Domain.
// https://creativecommons.org/publicdomain/zero/1.0/

// #![allow(unused)]

// use core::time::Duration;
// use tokio::time::sleep as sleep;

use lego_powered_up::IoDevice;
// use lego_powered_up::consts::MotorSensorMode;
use lego_powered_up::consts::named_port;
use lego_powered_up::iodevice::modes;
use lego_powered_up::iodevice::motor::EncoderMotor;
use lego_powered_up::iodevice::remote::{RcButtonState, RcDevice};
use lego_powered_up::iodevice::sensor::GenericSensor;
use lego_powered_up::notifications::Power;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (main_hub, rc_hub) = lego_powered_up::setup::main_and_rc().await?;

    // Set up RC input
    let rc: IoDevice;
    {
        let lock = rc_hub.mutex.lock().await;
        rc = lock.io_from_port(named_port::A)?;
        // rc = lock.io_from_port(16).await?;
    }
    let (mut rc_rx, _rc_task) = rc.remote_connect_with_green()?;

    // Set up motor feedback
    let motor: IoDevice;
    {
        let lock = main_hub.mutex.lock().await;
        motor = lock.io_from_port(named_port::A)?;
    }
    let (mut motor_rx, _position_task) = motor
        .enable_32bit_sensor(modes::InternalMotorTacho::POS, 1)
        .await?;

    // Control task
    let motor_control = tokio::spawn(async move {
        // const MAX_POWER: Power = Power::Cw(100);
        const MAX_POWER: u8 = 100;
        let mut set_limit: (Option<i32>, Option<i32>) = (None, None);
        let mut set_speed: i8 = 20;
        let mut pos: i32 = 0;
        let mut at_limit: (bool, bool) = (false, false);
        let mut cmd: (bool, bool) = (false, false);
        loop {
            tokio::select! { biased;
                Ok(msg) = motor_rx.recv() => {
                    pos = msg[0];
                    match set_limit.0 {
                        None => (),
                        Some(limit) => {
                            if (pos <= limit) & cmd.0 {
                                let _ = motor.start_power(Power::Brake);
                                at_limit.0 = true;
                                println!("Left LIMIT: {}", limit);
                            } else {
                                at_limit.0 = false;
                            }
                        }
                    }
                    match set_limit.1 {
                        None => (),
                        Some(limit) => {
                            if (pos >= limit) & cmd.1 {
                                let _ = motor.start_power(Power::Brake);
                                at_limit.1 = true;
                                println!("Right LIMIT: {}", limit);
                            } else {
                                at_limit.1 = false;
                            }

                        }
                    }
                    println!("Pos: {}", pos);
                }
                Ok(msg) = rc_rx.recv() => {
                    match msg {
                        RcButtonState::Aup => {
                            let _ = motor.start_power(Power::Brake);
                            cmd = (false, false);
                        }
                        RcButtonState::Aminus => {
                            if !at_limit.0 {
                                cmd.0 = true;
                                let _ = motor.start_speed(-set_speed, MAX_POWER);
                            }
                        }
                        RcButtonState::Aplus => {
                            if !at_limit.1 {
                                cmd.1 = true;
                                let _ = motor.start_speed(set_speed, MAX_POWER);
                            }
                        }
                        RcButtonState::Ared => {
                            match set_limit.0 {
                                None => {
                                    set_limit.0 = Some(pos);
                                    println!("Left limit set: {:?}", pos);
                                }
                                Some(_) => {
                                    set_limit.0 = None;
                                    println!("Left limit cancelled");
                                }
                            }
                        }
                        RcButtonState::Bred => {
                            match set_limit.1 {
                                None => {
                                    set_limit.1 = Some(pos);
                                    println!("Right limit set: {:?}", pos);
                                }
                                Some(_) => {
                                    set_limit.1 = None;
                                    println!("Right limit cancelled");
                                }
                            }
                        }
                        RcButtonState::Bplus => {
                            if set_speed < 96 {
                                set_speed += 5;
                                println!("Set speed: {}", set_speed);
                            }
                        }
                        RcButtonState::Bminus => {
                            if set_speed > 4 {
                                set_speed -= 5;
                                println!("Set speed: {}", set_speed);
                            }
                        }

                        // RcButtonState::Bup => { println!("B side released"); }
                        RcButtonState::Green => {
                            // println!("Reset position");
                            // motor.preset_encoder(0).await;
                            println!("Exiting");
                            break;
                        }
                        // RcButtonState::GreenUp => { println!("Green released") }
                        _ => ()
                    }
                }
                else => { break }
            };
        }
    });

    motor_control.await?;

    // Cleanup
    println!("Disconnect from hub `{}`", rc_hub.name);
    {
        let lock = rc_hub.mutex.lock().await;
        lock.disconnect().await?;
    }
    println!("Disconnect from hub `{}`", main_hub.name);
    {
        let lock = main_hub.mutex.lock().await;
        lock.disconnect().await?;
    }

    println!("Done!");

    Ok(())
}
