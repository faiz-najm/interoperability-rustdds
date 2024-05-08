use std::{io, thread, time};
use std::task::Poll;
use std::thread::Thread;
use std::time::Duration;

use rustdds::{DomainParticipant, policy, QosPolicyBuilder, Duration as DDSDuration, CDRDeserializerAdapter, CDRSerializerAdapter, StatusEvented, TopicKind};
use log4rs::{
    append::console::ConsoleAppender,
    config::{Appender, Root},
    Config,
};
use log::LevelFilter;
use futures::StreamExt;
use rustdds::policy::{Durability, Presentation, Reliability};
use smol::Timer;

const SECOND: Duration = Duration::from_millis(1000);

pub mod Messenger {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Message {
        pub from: String,
        pub subject: String,
        pub subject_id: i32,
        pub text: String,
        pub count: i32,
    }
}

fn main() {
    // DomainParticipant is always necessary
    let domain_participant = DomainParticipant::new(0)
        .unwrap_or_else(|e| panic!("DomainParticipant construction failed: {e:?}"));

    let qos = QosPolicyBuilder::new().build();

    // DDS Subscriber, only one is necessary for each thread (slight difference to
    // DDS specification)
    let subscriber = domain_participant.create_subscriber(&qos).unwrap();

    // DDS Publisher, only one is necessary for each thread (slight difference to
    // DDS specification)
    let publisher = domain_participant.create_publisher(&qos).unwrap();

    // Some DDS Topic that we can write and read from (basically only binds readers
    // and writers together)
    // Used type needs Serialize for writers and Deserialize for readers


    println!("Input 'false' to create a publisher or 'true' to create a subscriber");

    let mut role = String::new();

    io::stdin()
        .read_line(&mut role)
        .expect("Failed to read line");

    let role: bool = match role.trim().parse() {
        Ok(role) => role,
        Err(_) => return,
    };

    if role {
        println!("Creating a subscriber");
        // Creating DataReader requires type and deserializer adapter (which is
        // recommended to be CDR).
        println!("Creating topic...");
        let some_topic = domain_participant
            .create_topic(
                "topics_msg".to_string(),
                "Messenger::Message".to_string(),
                &qos,
                TopicKind::NoKey,
            ).expect("Error: Topic creation failed");
        println!("Topic: {:?} created", &some_topic);

        let reader = subscriber
            .create_datareader_no_key_cdr::<Messenger::Message>(&some_topic, None)
            .unwrap();

        println!("Subscriber created");

        smol::block_on(async {
            let mut datareader_stream = reader.async_sample_stream();
            let mut datareader_event_stream = datareader_stream.async_event_stream();

            loop {
                futures::select! {
          r=datareader_stream.select_next_some()=>{
            match r{
              Ok(d)=>{println!("{}",d.text)},
              Err(e)=> {
                println!("{:?}", e);
                break;
              }
            }
          }
          e=datareader_event_stream.select_next_some()=>{
            println!("DataReader event: {e:?}");
          }
        }
            }
        })
    } else {
        println!("Creating a publisher");
        // Creating DataWriter required type and serializer adapter (which is
        // recommended to be CDR).
        println!("Finding topic...");
        let some_topic = domain_participant
            .find_topic(
                "topics_msg",
                Duration::from_secs(7),
            ).unwrap_or_else(|e| {
            println!("Error: {:?}", e);
            std::process::exit(1);
        }).unwrap_or_else(|| {
            println!("Error: Topic not found");
            std::process::exit(1);
        });
        println!("Found topic: {:?}", &some_topic);

        let writer = publisher
            .create_datawriter_no_key::<Messenger::Message, CDRSerializerAdapter<Messenger::Message>>(&some_topic, None)
            .unwrap();

        smol::block_on(async {
            let mut tick_stream = futures::StreamExt::fuse(Timer::interval(SECOND));

            let mut datawriter_event_stream = writer.as_async_status_stream();

            let mut i = 0;

            loop {
                futures::select! {
          _= tick_stream.select_next_some()=> {
            let some_data = Messenger::Message {
                from: "Rust".to_string(),
                subject: "Hello DDS".to_string(),
                subject_id: 0,
                text: "Hello DDS World".to_string(),
                count: i, };
                        i += 1;
                        println!("DataWriter write: {:#?}", &some_data);
                        writer.async_write(some_data,None).await
                        .unwrap_or_else(|e|
                            println!("DataWriter write failed: {e:?}"));

          }
          e= datawriter_event_stream.select_next_some()=>{
            println!("DataWriter event: {e:?}");
          }
        }
            }
        })
    }
}

// fn configure_logging() {
//     // initialize logging, preferably from config file
//     log4rs::init_file(
//         "logging-config.yaml",
//         log4rs::config::Deserializers::default(),
//     )
//         .unwrap_or_else(|e| {
//             match e.downcast_ref::<io::Error>() {
//                 // Config file did not work. If it is a simple "No such file or directory", then
//                 // substitute some default config.
//                 Some(os_err) if os_err.kind() == io::ErrorKind::NotFound => {
//                     println!("No config file found in current working directory.");
//                     let stdout = ConsoleAppender::builder().build();
//                     let conf = Config::builder()
//                         .appender(Appender::builder().build("stdout", Box::new(stdout)))
//                         .build(Root::builder().appender("stdout").build(LevelFilter::Error))
//                         .unwrap();
//                     log4rs::init_config(conf).unwrap();
//                 }
//                 // Give up.
//                 other_error => panic!("Config problem: {other_error:?}"),
//             }
//         });
// }