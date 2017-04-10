
use ws;

use url::Url;

use std::error::Error;
use std::{thread, time};
use std::collections::VecDeque;

use time::Tm;

use user;
use channel;
use message;
use rfc;
use irc;
use subscriber;
use util;

static DEFAULT_COMMAND_PREFIX: &'static str = "!";
static DEFAULT_HOST: &'static str = "irc-ws.chat.twitch.tv";
const DEFAULT_PORT: i32 = 80;

pub struct TwitchClient {
	out: ws::Sender,

	credentials: ConnectionCredentials,
	default_channel: String,
	logging: bool,

	channels: Vec<channel::Channel>,
	channel_queue: VecDeque<channel::Channel>,
	currently_joining: bool,

	cmd_idents: Vec<String>,

	on_event: fn(&TwitchClient, Event),
}

impl TwitchClient {
	pub fn out(&self) -> ws::Sender {
		self.out
	}

	pub fn credentials(&self) -> ConnectionCredentials {
		self.credentials
	}

	pub fn default_channel(&self) -> &str {
		self.default_channel.as_str()
	}
}

impl TwitchClient {
	pub fn connect(
		user: &str,
		auth: &str,
		channel: &str,
		log: bool,
		cmd_idents: &[&str],
		on_event: fn(&TwitchClient, Event),
	) -> ws::Result<()> {

		if log {
			println!("twitch-rs initialized. StringAttempting to connect to twitch");
		}

		// TODO: add command identifiers for chat and whispers

		let creds = ConnectionCredentials::from(user, auth);

		ws::connect(
			format!("ws://{}:{}", creds.host(), creds.port()),
			move |out| {
				TwitchClient {
					out: out,

					credentials: creds,
					default_channel: channel.to_string(),
					logging: log,

					channels: Vec::new(),
					channel_queue: VecDeque::new(),
					currently_joining: false,

					cmd_idents: cmd_idents.into_iter().map(|x| x.to_string()).collect(),

					on_event: on_event,
				}
			}
		)
	}

	pub fn web_send(&self, msg: &str) -> ws::Result<()> {
		println!("Sending: {}", msg);
		self.out.send(msg)
	}

	pub fn parse_irc(&mut self, irc: &str) {

		let mut msg_found = false;

		// on connected
		if irc::irc_connected(&irc) {
			(self.on_event)(self, Event::Connected(
				self.credentials().user().clone(),
				self.default_channel.clone()
			));
			return;
		}

		// on new sub
		match irc::irc_new_subscriber(&irc, &self.channels) {
			Some(channel) => {
				(self.on_event)(self, Event::NewSubscriber(
					channel,
					subscriber::NewSubscriber::from(irc)
				));
				return;
			}
			_ => {}
		}

		// on message received
		match irc::irc_message_received(&irc, &self.channels) {
			Some(channel) => {
				msg_found = true;
				println!("received!!!");
				(self.on_event)(self, Event::MessageReceived(
					message::ChatMessage::from(
						self.credentials().user().clone(),
						irc.clone()
					)
				));
				println!("oh boy...");
			}
			_ => {}
		}

		println!("command coming up");

		// on command received
		if self.channels.iter().any(|chan| {
			self.cmd_idents.iter().any(|cmd| {
				irc::irc_command_received(
					self.credentials().user(),
					&irc,
					chan.name(),
					cmd.as_str()
				)
			})
		}) {
			let msg = message::ChatMessage::from(
					self.credentials().user(),
					irc
				);
			let cmd = if msg.text().split(" ").nth(0).unwrap_or("") != "" {
				util::sub(
					msg.text().split(" ").nth(0).unwrap_or(""),
					1,
					msg.text().split(" ").nth(0).unwrap_or("").len() - 1
				)
			}
			else {
				util::sub(msg.text(), 1, msg.text().len() - 1)
			};

			let cmd_prefix =
				self.cmd_idents.iter()
					.filter(|&x| msg.text().starts_with(x))
					.next()
					.unwrap_or(&DEFAULT_COMMAND_PREFIX.to_string());
			
			let args_string = 
				if msg.text().contains(" ") &&
					msg.text().split(" ").nth(0).unwrap_or("") != ""
				{
					msg.text().replace(
						&format!("{} ", msg.text().split(" ").nth(0).unwrap_or("")),
						""
					).as_str()
				}
				else {
					""
				}
			;

			let args: &[&str] =
			if !msg.text().contains("\"") ||
				msg.text().chars().map(|x| x == '"').count() % 2 == 1
			{

				&msg.text().split(" ")
					.filter(|arg| {
						arg != &format!("{}{}", cmd_prefix, cmd)
					}).collect::<Vec<_>>()
			}
			else {
				util::args_with_quotes(&args_string)
			};
			
			(self.on_event)(self, Event::ChatCommandReceived(
				msg,
				cmd,
				args,
				args_string,
				cmd_prefix
			));
				
			return;
		}
		
		/*if irc::irc_command_received(
			self.credentials().user(),
			&irc,
			&self.channels,
			&self.cmd_idents.iter().map(|x| x.as_str()).collect::<Vec<_>>()[..]
		) {
			Some(channel) => {
				let msg = message::ChatMessage::from(
					self.credentials().user(),
					irc
				);
				let cmd = if msg.text().split(" ").nth(0).unwrap_or("") != "" {
					util::sub(
						msg.text().split(" ").nth(0).unwrap_or(""),
						1,
						msg.text().split(" ").nth(0).unwrap_or("").len() - 1
					)
				}
				else {
					util::sub(msg.text(), 1, msg.text().len() - 1)
				};

				let cmd_prefix =
					self.cmd_idents.iter()
						.filter(|&x| msg.text().starts_with(x))
						.next()
						.unwrap_or(&DEFAULT_COMMAND_PREFIX.to_string());
				
				let args_string = 
					if msg.text().contains(" ") &&
						msg.text().split(" ").nth(0).unwrap_or("") != ""
					{
						msg.text().replace(
							&format!("{} ", msg.text().split(" ").nth(0).unwrap_or("")),
							""
						).as_str()
					}
					else {
						""
					}
				;

				let args: &[&str] =
				if !msg.text().contains("\"") ||
					msg.text().chars().map(|x| x == '"').count() % 2 == 1
				{

					&msg.text().split(" ")
						.filter(|arg| {
							arg != &format!("{}{}", cmd_prefix, cmd)
						}).collect::<Vec<_>>()
				}
				else {
					util::args_with_quotes(&args_string)
				};
				
				(self.on_event)(self, Event::ChatCommandReceived(
					msg,
					cmd,
					args,
					args_string,
					cmd_prefix
				));
					
				return;
			}
			_ => {}
		}*/

		if msg_found {
			println!("found message, leaving parse_irc");
			return;
		}

		// on user joined
		match irc::irc_user_joined(&irc, &self.channels) {
			Some(channel) => {
				if self.credentials().user().to_lowercase() ==
					irc.split("!").nth(1).unwrap_or("")
					.split("@").nth(0).unwrap_or("").to_lowercase()
				{
					(self.on_event)(self, Event::ChannelJoined(
						channel,
						irc.split("!").nth(1).unwrap_or("")
							.split("@").nth(0).unwrap_or("")
					));
				}
				else {
					(self.on_event)(self, Event::UserJoined(
						self.credentials().user().clone(),
						channel
					));
				}
				return;
			}
			_ => {}
		}

		// on user left
		match irc::irc_user_left(&irc, &self.channels) {
			Some(channel) => {
				let username = irc.split(":").nth(1).unwrap_or("")
								.split("!").nth(0).unwrap_or("");
				if username.to_lowercase() == self.credentials().user().to_lowercase() {
					
					let pos = self.channels.iter().position(|x| {
						x.name().to_lowercase() == channel.to_lowercase()
					}).unwrap_or(0);
					self.channels.remove(pos);
					// TODO: do we need _hasSeenJoinedChannels? (see TwitchClient.cs:647)
					(self.on_event)(self, Event::UserLeft(
						username,
						channel
					));
				}
				else {
					(self.on_event)(self, Event::UserLeft(
						username,
						channel
					));
				}
				return;
			}
			_ => {}
		}

		// on moderator joined
		match irc::irc_moderator_joined(&irc, &self.channels) {
			Some(channel) => {
				(self.on_event)(self, Event::ModeratorJoined(
					irc.split(" ").nth(4).unwrap_or(""),
					channel
				));
				return;
			}
			_ => {}
		}

		// on moderator left
		match irc::irc_moderator_left(&irc, &self.channels) {
			Some(channel) => {
				(self.on_event)(self, Event::ModeratorLeft(
					irc.split(" ").nth(4).unwrap_or(""),
					channel
				));
				return;
			}
			_ => {}
		}

		// on incorrect login
		if irc::irc_incorrect_login(&irc) {
			self.disconnect();

			(self.on_event)(self, Event::IncorrectLogin(
				"Invalid username or password/oauth"
			));
			return;
		}

		// on malformed oauth
		match irc::irc_malformed_oauth(&irc, &self.channels) {
			Some(_) => {
				self.disconnect();

				(self.on_event)(self, Event::IncorrectLogin(
					"Invalid OAuth key. Remember to add 'oauth:' as a prefix."
				));
				return;
			}
			_ => {}
		}

		// on host left
		match irc::irc_host_left(&irc, &self.channels) {
			Some(_) => {
				(self.on_event)(self, Event::HostLeft(
					None
				));
			}
			_ => {}
		}

		// on channel state changed
		match irc::irc_channel_state_changed(&irc, &self.channels) {
			Some(channel) => {
				(self.on_event)(self, Event::ChannelStateChanged(
					channel::ChannelState::from(irc.clone()),
					channel
				));
			}
			_ => {}
		}

		// on user state changed
		match irc::irc_user_state_changed(&irc, &self.channels) {
			Some(channel) => {
				// TODO: Implement state change
				return;
			}
			_ => {}
		}

		// on resubscriber
		match irc::irc_re_subscriber(&irc, &self.channels) {
			Some(channel) => {
				(self.on_event)(self, Event::ReSubscriber(
					channel,
					subscriber::ReSubscriber::from(irc.clone())
				));
				return;
			}
			_ => {}
		}

		// on ping received
		if irc::irc_ping(&irc) {
			self.web_send("PONG");
			return;
		}

		// on pong received (do nothing but return)
		if irc::irc_pong(&irc) {
			return;
		}

		// on hosting stopped
		if irc::irc_hosting_stopped(&irc) {
			let viewers = irc.split(" ").nth(4).unwrap_or("0").parse::<i32>();
			(self.on_event)(self, Event::HostingStopped(
				viewers.unwrap_or(0),
				util::sub(
					irc.split(" ").nth(2).unwrap_or(""),
					2,
					irc.split(" ").nth(2).unwrap_or("").len()
				)
			));
			return;
		}

		// on hosting started
		if irc::irc_hosting_started(&irc) {
			let viewers = irc.split(" ").nth(4).unwrap_or("0").parse::<i32>();
			(self.on_event)(self, Event::HostingStarted(
				viewers.unwrap_or(0),
				util::sub(
					irc.split(" ").nth(2).unwrap_or(""),
					2,
					irc.split(" ").nth(2).unwrap_or("").len()
				),
				util::sub(
					irc.split(" ").nth(3).unwrap_or(""),
					2,
					irc.split(" ").nth(2).unwrap_or("").len()
				)
			));
			return;
		}

		// on existing users detected
		match irc::irc_existing_users(&irc, self.credentials().user(), &self.channels) {
			Some(channel) => {
				(self.on_event)(self, Event::ExistingUsersDetected(
					channel.clone(),
					irc.replace(
						&format!(
							":{user}.tmi.twitch.tv 353 {user} = #{chan} :",
							user = self.credentials().user(), chan = channel
						),
						""
					).split(" ").collect()
				));
				return;
			}
			_ => {}
		}

		// on now hosting
		match irc::irc_now_hosting(&irc, &self.channels) {
			Some(channel) => {
				(self.on_event)(self, Event::NowHosting(
					channel,
					irc.split(" ").nth(6).unwrap_or("").replace(".", "").as_str()
				));
				return;
			}
			_ => {}
		}

		// on channel join
		match irc::irc_join_channel_completed(&irc) {
			Some(channel) => {
				self.currently_joining = true;
				self.check_join_queue();
				return;
			}
			_ => {}
		}

		// TODO: handle irc commands here

		self.log(format!("Unaccounted for: {}", irc));
	}

	pub fn join_channel(&mut self, channel: &str) {
		// TODO: Join channel
		let chan = channel.to_lowercase();
		let chan_names: Vec<String> = self.channels.iter().map(|c| c.name().to_string()).collect::<Vec<String>>();

		if chan_names.contains(&channel.to_lowercase()) {
			return;
		}

		self.channel_queue.push_back(channel::Channel::from(
			channel
		));

		if !self.currently_joining {
			self.check_join_queue();
		}
	}

	pub fn send_jc_message(&self, channel: &channel::Channel, message: &str) {
		// TODO: Check if messages are throttled (refer to TwitchClient.cs:297)

		self.web_send(format!(
			":{user}!{user}@{user}.tmi.twitch.tv PRIVMSG #{chan} :{msg}",
			user = self.credentials().user(),
			chan = channel.name(),
			msg = message
		).as_ref());

	}

	pub fn send_channel_message(&self, channel: &str, message: &str) {
		self.send_jc_message(self.channel_from_string(channel).unwrap(), message);
	}

	pub fn send_message(&self, message: &str) {
		match self.channels.first() {
			Some(channel) => self.send_jc_message(channel, message),
			None => {}
		};
	}

	pub fn send_whisper(&self, receiver: &str, message: &str) {

		// TODO: Check if whispers are throttled (refer to TwitchClient.cs:333)

		self.web_send(format!(
			":{user}~{user}@{user}.tmi.twitch.tv PRIVMSG #jtv :/w {r} {m}",
			user = self.credentials().user(),
			r = receiver,
			m = message
		).as_ref());
		(self.on_event)(self, Event::WhisperSent(
			receiver,
			message
		));
	}

	pub fn channel_from_string(&self, channel: &str) -> Result<&channel::Channel, ()> {
		for (index, chan) in self.channels.iter().enumerate() {
			if chan.name().to_lowercase() == channel.to_lowercase() {
				return Ok(&self.channels[index]);
			}
		}
		Err(())
	}

	pub fn check_join_queue(&mut self) {
		if !self.channel_queue.is_empty() {
			self.currently_joining = true;
			match self.channel_queue.pop_back() {
				Some(chan) => {
					self.log(format!("Joining channel: {}", chan.name()));
					self.web_send(rfc::join_channel(
						&format!("#{}", chan.name())
					));
					self.channels.push(chan.clone());
				}
				_ => {}
			}
		}
		else {
			self.log(String::from("Finished channel joining queue."));
		}
	}

	pub fn disconnect(&mut self) -> ws::Result<()> {
		self.log(String::from("Disconnect Twitch Chat Client..."));

		self.out.close(ws::CloseCode::Abnormal)?;

		self.channels.clear();

		Ok(())
	}

	pub fn reconnect(&self) -> ws::Result<()> {
		self.log(format!(
			"Reconnecting to: {}:{}",
			self.credentials().host(),
			self.credentials().port()
		));

		self.out.close(ws::CloseCode::Abnormal);
		let url = Url::parse(format!(
			"ws://{}:{}",
			self.credentials().host().clone(),
			self.credentials().port().clone()
		).as_str()).unwrap();
		self.out.connect(url)
	}

	fn log(&self, message: String) {
		if self.logging  {
			println!("{}", message);
		}
	}

	fn on_web_open(&mut self, hs: ws::Handshake) -> ws::Result<()> {

		self.log(format!("Connected to twitch, passing credentials"));
		self.web_send(
			rfc::pass(self.credentials().auth())
		)?;
		self.web_send(
			rfc::nick(self.credentials().user())
		)?;
		self.web_send(
			rfc::user(
				self.credentials().user(),
				&0,
				self.credentials().user()
			)
		)?;

		self.web_send("CAP REQ twitch.tv/membership")?;
		self.web_send("CAP REQ twitch.tv/commands")?;
		self.web_send("CAP REQ twitch.tv/tags")?;

		if !self.default_channel().is_empty() {
			let chan = self.default_channel().clone();
			self.join_channel(&chan);
		}

		Ok(())
	}

	fn on_web_message(&mut self, msg: ws::Message) -> ws::Result<()> {

		println!("Received web message");

		let is_text = msg.is_text();
		let data = msg.into_text().unwrap_or(String::from(""));
		let lines = data.split("\n");

		for line in lines {
			if line.len() > 1 {
				self.log(format!("Received: {}", line));
				if is_text {
					(self.on_event)(self, Event::SendReceiveData(
						SendReceiveDirection::Received,
						line.clone()
					));
					self.parse_irc(line);
				}
			}
		}


		Ok(())
	}

	fn on_web_close(&mut self, code: ws::CloseCode, reason: &str) {

		self.log(format!("The connection was closed: {}", reason));

		(self.on_event)(self, Event::Disconnected(
			self.credentials().user().clone(),
			reason
		));
		self.channels.clear();
	}

	fn on_web_error(&mut self, err: ws::Error) {

		self.log(format!("There was an error: {}", err.description()));

		self.reconnect();

		thread::sleep(time::Duration::from_secs(2));

		(self.on_event)(self, Event::ConnectionError(
			self.credentials().user().clone(),
			err.description()
		));
	}
}

impl ws::Handler for TwitchClient {

	fn on_open(&mut self, hs: ws::Handshake) -> ws::Result<()> {
		self.on_web_open(hs)
	}

	fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
		self.on_web_message(msg)
	}

	fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
		self.on_web_close(code, reason)
	}

	fn on_error(&mut self, err: ws::Error) {
		self.on_web_error(err)
	}
}

pub struct ConnectionCredentials {
	user: String,
	auth: String,
	host: String,
	port: i32,
}

impl ConnectionCredentials {

	pub fn from(user: &str, auth: &str) -> ConnectionCredentials {
		ConnectionCredentials::from_host(user, auth, DEFAULT_HOST, DEFAULT_PORT)
	}

	pub fn from_host(
		user: &str,
		auth: &str,
		host: &str,
		port: i32
	) -> ConnectionCredentials {

		// we're offloading this oauth fix here
		// since it doesnt need to be in on_open/OnConnected

		let mut auth = auth;

		if !auth.contains(":") {
			auth = format!("oauth:{}", auth.replace("oauth", "")).as_str();
		}

		ConnectionCredentials {
			user: user.to_string(),
			auth: auth.to_string(),
			host: host.to_string(),
			port: port,
		}
	}

	pub fn user(&self) -> &str {
		self.user.as_str()
	}

	pub fn auth(&self) -> &str {
		self.auth.as_str()
	}

	pub fn host(&self) -> &str {
		self.host.as_str()
	}

	pub fn port(&self) -> i32 {
		self.port
	}
}

impl Clone for ConnectionCredentials {
	fn clone(&self) -> ConnectionCredentials {
		ConnectionCredentials {
			user: self.user,
			auth: self.auth,
			host: self.host,
			port: self.port
		}
	}
}