using Pkg

function get_commit_id()
  event_file = get(ENV, "GITHUB_EVENT_PATH", "")
  ref = "HEAD"
  gaction = get(ENV, "GITHUB_ACTIONS", "")
  if !isempty(gaction)
    # .pull_request.head.sha, .release.tag_name,
    ref = readlines(`jq --raw-output '.pull_request.head.sha' $event_file`)[1]
    if ref == "null"
      ref = readlines(`jq --raw-output '.release.tag_name' $event_file`)[1]
    end
  end

  if ref == "null"
    ref = "HEAD"
  end

  return readlines(`git rev-parse $ref`)[1]
end

function get_version()
  version = VersionNumber(strip(Pkg.TOML.parsefile(joinpath(@__DIR__, "../Cargo.toml"))["package"]["version"]))
  string(version)
end

function main()
  filename = ARGS[1]

  # Read the original file
  contents = read(filename, String)

  # Define patterns
  version_pattern = r"version = v\"[\d\.]*\""
  hash_pattern = r"\"[0-9a-f]{40}\""

  new_version = get_version()
  new_commit_hash = get_commit_id()

  # Update version and commit hash
  new_contents = replace(contents, version_pattern => "version = v\"$new_version\"")
  new_contents = replace(new_contents, hash_pattern => "\"$new_commit_hash\"")

  # Write the updated contents back to the file
  write(filename, new_contents)
end

main()
