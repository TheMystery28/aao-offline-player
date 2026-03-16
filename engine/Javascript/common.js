function defineGET()
{
	_GET = new Object();
	
	if(location.href.match(/\?/))
	{
		var elts = location.href.replace(/^([^?]*\?)/, '').split('&');
		
		for(var i = 0; i < elts.length; i++)
		{
			var subelts = elts[i].split('=');
			if(subelts.length > 1)
			{
				_GET[decodeURIComponent(subelts[0])] = decodeURIComponent(subelts[1]);
			}
			else
			{
				_GET[decodeURIComponent(subelts[0])] = null;
			}
		}
	}
}

var _GET;

defineGET();

/*
Get the file version to use for code resources.
*/
function getFileVersion(path_components)
{
	if(typeof file_versions != 'object')
	{
		// If the global file_versions definition is not available, assume file exists and try to force to use latest version of the target file.
		return new Date();
	}
	
	var parent_object = file_versions;
	while(path_components.length > 0)
	{
		var path_component = path_components.shift();
		
		if(!(path_component in parent_object))
		{
			// If the target file does not exist, return null.
			return null;
		}
		parent_object = parent_object[path_component];
	}
	
	return parent_object;
}
