use crate::config::{Config, geode_root};
use crate::input::ask_value;
use std::fs;
use std::path::PathBuf;
use git2::{Repository, ResetType, IndexAddOption, Signature};
use clap::Subcommand;
use crate::package::mod_json_from_archive;
use crate::{info, warn, done, fatal};
use colored::Colorize;

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Indexer {
	/// Initializes your indexer
	Init,

	/// Lists all entries in your indexer
	List,

	/// Removes an entry from your indexer
	Remove {
		/// Mod ID that you want to remove
		id: String
	},

	/// Exports an entry to your indexer, updating if it always exists
	Export {
		/// Path to the .geode file
		package: PathBuf
	}
}

fn reset_and_commit(repo: &Repository, msg: &str) {
	let head = repo.head().expect("Broken repository, can't get HEAD");
	if !head.is_branch() {
		fatal!("Broken repository, detached HEAD");
	}

	let mut commit = head.peel_to_commit().unwrap();
	while commit.parent_count() > 0 {
		commit = commit.parent(0).unwrap();
	}

	repo.reset(commit.as_object(), ResetType::Soft, None).expect("Unable to refresh repository");
	
	let mut index = repo.index().expect("cannot get the Index file");
	index.add_all(["."].iter(), IndexAddOption::DEFAULT, None).expect("Unable to add changes");
	index.write().expect("Unable to write changes");

	let sig = Signature::now("GeodeBot", "hjfodgames@gmail.com").unwrap();

	let tree = repo.find_tree(index.write_tree().expect("Unable to get write tree")).unwrap();
	repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&commit]).expect("Unable to commit");
}

fn initialize() {
	let indexer_path = geode_root().join("indexer");
	if indexer_path.exists() {
		warn!("Indexer is already initialized. Exiting.");
		return;
	}

	info!("Welcome to the Indexer Setup. Here, we will set up your indexer to be compatible with the Geode index.");
	info!("Before continuing, make a github fork of https://github.com/geode-sdk/indexer.");

	let fork_url = ask_value("Enter your forked URL", None, true);
	Repository::clone(&fork_url, indexer_path).expect("Unable to clone your repository.");

	done!("Successfully initialized");
}

fn list_mods() {
	let indexer_path = geode_root().join("indexer");
	if !indexer_path.exists() {
		fatal!("Indexer has not yet been initialized.");
	}

	println!("Mod list:");

	for dir in fs::read_dir(indexer_path).unwrap() {
		let path = dir.unwrap().path();

		if path.is_dir() && path.join("mod.geode").exists() {
			println!("    - {}", path.file_name().unwrap().to_str().unwrap().bright_green());
		}
	}
}

fn remove_mod(id: String) {
	let indexer_path = geode_root().join("indexer");
	if !indexer_path.exists() {
		fatal!("Indexer has not yet been initialized.");
	}

	let mod_path = indexer_path.join(&id);
	if !mod_path.exists() {
		fatal!("Cannot remove mod {}: does not exist", id);
	}

	fs::remove_dir_all(mod_path).expect("Unable to remove mod");

	let repo = Repository::open(&indexer_path).expect("Unable to open repository");
	reset_and_commit(&repo, &format!("Remove {}", &id));

	done!("Succesfully removed {}\n", id);
	info!("You will need to force-push this commit yourself. Type: ");
	info!("git -C {} push -f", indexer_path.to_str().unwrap());
}

fn export_mod(package: PathBuf) {
	let indexer_path = geode_root().join("indexer");
	if !indexer_path.exists() {
		fatal!("Indexer has not yet been initialized.");
	}

	if !package.exists() {
		fatal!("Path not found");
	}

	let mut archive = zip::ZipArchive::new(fs::File::open(&package).unwrap()).expect("Unable to read package");
	
	let mod_json = mod_json_from_archive(&mut archive);

	let major_version = mod_json
		.get("version")
		.expect("[mod.json]: Missing key 'version'")
		.as_str()
		.expect("[mod.json].version: Expected string")
		.split(".")
		.next()
		.unwrap()
		.chars()
		.filter(|x| *x != 'v')
		.collect::<String>();

	let mod_id = mod_json_from_archive(&mut archive)
		.get("id")
		.expect("[mod.json]: Missing key 'id'")
		.as_str()
		.expect("[mod.json].id: Expected string")
		.to_string();

	let mod_path = indexer_path.join(format!("{}@{}", &mod_id, &major_version));
	if !mod_path.exists() {
		fs::create_dir(&mod_path).expect("Unable to create folder");
	}

	fs::copy(package, mod_path.join("mod.geode")).expect("Unable to copy mod");

	let repo = Repository::open(&indexer_path).expect("Unable to open repository");
	reset_and_commit(&repo, &format!("Add/Update {}", &mod_id));

	done!("Successfully exported {}@{} to your indexer\n", mod_id, major_version);
	
	info!("You will need to force-push this commit yourself. Type: ");
	info!("git -C {} push -f", indexer_path.to_str().unwrap());
}


pub fn subcommand(_config: &mut Config, cmd: Indexer) {
	match cmd {
		Indexer::Init => initialize(),
		
		Indexer::List => list_mods(),

		Indexer::Remove { id } => remove_mod(id),

		Indexer::Export { package } => export_mod(package)
	}
}