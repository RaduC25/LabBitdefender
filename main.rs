use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Seek},
};

#[derive(Debug,Serialize,Deserialize)]
struct FileData{
    name : String,
    filenames : Vec<String>,
}

fn list_zip_contents(reader: impl Read + Seek) -> zip::result::ZipResult<Vec<String>> {
    let mut zip = zip::ZipArchive::new(reader)?;
    let mut vector:Vec<String>=Vec :: new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        vector.push(file.name().to_string());
    }
    return Ok(vector);

}
use serde::{Deserialize, Serialize};
use std::io::Write;
use serde_json;

fn serialize_to<W: Write, T: ?Sized + Serialize>(mut writer: W, value: &T) -> Result<(),std::io::Error> {

    serde_json::to_writer(&mut writer, value)?;
    writer.write_all(b"\n")
}


use std::error::Error;
use std::io::BufReader;
use std::path::Path;

//xc#[derive(Deserialize, Debug)]
/*struct User {
    fingerprint: String,
    location: String,
}

fn read_user_from_file<P: AsRef<Path>>(path: P) -> Result<User, Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `User`.
    let u = serde_json::from_reader(reader)?;

    // Return the `User`.
    Ok(u)
}*/




fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args: Vec<String> = std::env::args().collect();
    
    let dir = &args[1];
    let mut zip_files:Vec<FileData> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let filename =path.file_name().unwrap().to_string_lossy().to_string();

        if path.is_file() && path.extension() == Some(OsStr::new("zip")) {
            let file = File::open(&path)?;
            let vec = list_zip_contents(file)?;
            //et name: = path.display().to_string();
            //let filename = list_zip_contents(file)?;
            //println!("{:?}",vec);
            /*for i in vec.iter(){

                println!("{:?}",i);
            }*/
            zip_files.push(FileData{
                name: filename,
                filenames: vec,
            });


            //println!("Contents of {:?}:", path);
            //list_zip_contents(file)?;

            
        } else {
            println!("Skipping {:?}", path);
        }
    }
    let json_file = File::create("file.ndjson")?;
    for i in zip_files.iter(){
        serialize_to(&json_file,i)?;
    }

    /*let u = read_user_from_file("zip_data.json").unwrap();
    println!("{:#?}", u);*/

    Ok(())
}