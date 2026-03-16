"use strict";
/*
Ace Attorney Online - Expression engine
AAOv5 version - to be rewritten in the future
*/

//MODULE DESCRIPTOR
Modules.load(new Object({
	name : 'expression_engine',
	dependencies : ['var_environments'],
	init : function() {}
}));

//INDEPENDENT INSTRUCTIONS
//Algo de levenshtein pour difference entre deux chaines
function levenshtein(a, b)
{
	// Calculate Levenshtein distance between two strings  
	// discuss at: http://phpjs.org/functions/levenshtein
	
	var min=Math.min, len1=0, len2=0, I=0, i=0, d=[], c='', j=0, J=0;
    
	if (a == b) 
	{
		return 0;
	}
	if (!a.length || !b.length) 
	{
		return b.length || a.length;
	}
	
	len1 = a.length + 1;
	len2 = b.length + 1;
	d = [[0]];
	while (++i < len2)
	{
		d[0][i] = i;
	}
	i = 0;
	while (++i < len1) 
	{
		J = j = 0;
		c = a[I];
		d[i] = [i];
		while (++j < len2) 
		{
			d[i][j] = min(d[I][j] + 1, d[i][J] + 1, d[I][J] + (c != b[J]));
			++J;
		}
		++I;
	}
    
    return d[len1 - 1][len2 - 1];
}

function ExpressionEngine()
{
	/* type lexeme
		[0,int] pour un nombre
		[1,str] pour un op 1
		[2,str] pour un op 2
		[3, ''] pour une parg
		[4, ''] pour une pard
		[5, str, 0] pour une var en cours, [5, str, 1] si finie ou simple
		[6, str, 0] pour une str en cours, [6, str, 1] si finie
		[7, str, 0] pour une fct en cours, [7, str, 0] si finie
		[8, ''] pour une virgule
		
		
		[1000, ''] pour rien du tout
	*/
	
	this.decompose = function(s)
	{
		var l = new Array();
		var l_e;
		
		for(var i = 0; i < s.length; i++)
		{
			if(l.length > 0)
			{
				var l_e = l[l.length - 1];
			}
			else
			{
				var l_e = [1000, ''];
			}
			
			if(l_e[0] == 6 && l_e[2] == 0) //si on est en train de d�f une str
			{
				if(l_e[1][l_e[1].length - 1] == "\\") //si caract�re �chapp�
				{ 
					l_e[1] = l_e[1].substr(0, l_e[1].length - 1);
					l_e[1] = l_e[1]+s[i];
				}
				else if(s[i] == "'") //apos non �chap�e
				{
					l_e[2] = 1;
				}
				else //caract�re normal
				{
					l_e[1] = l_e[1]+s[i];
				}
			}
			else if(l_e[0] == 5 && l_e[2] == 0) //en train de d�f une var
			{
				if(l_e[1][l_e[1].length - 1] == "\\") //si caract�re �chapp�
				{ 
					l_e[1] = l_e[1].substr(0, l_e[1].length - 1);
					l_e[1] = l_e[1]+s[i];
				}
				else if(s[i] == '"') //dbl quote non �chap�e
				{
					l_e[2] = 1;
				}
				else //caract�re normal
				{
					l_e[1] = l_e[1]+s[i];
				}
			}
			else if(l_e[0] == 7 && l_e[2] == 0) //en train de d�finir une fct
			{
				if(s[i] == '(')
				{
					l_e[2] = 1;
					l.push([3, '']);
				}
				else
				{
					l_e[1] = l_e[1] + s[i];
				}
			}
			else
			{
				switch(s[i])
				{
					case ' ' : break;
					case '-' : case '!' :
						switch(l_e[0])
						{
							case 0 : case 4 : case 5 :
								l.push([2, s[i]]);
								break;
							default : l.push([1, s[i]]);
						}
						break;
					case "'" : l.push([6, '', 0]); break;
					case '"' : l.push([5, '', 0]); break;
					case '+' : case '*' : case '/' : case '^' : case '.' :
					case '&' : case '|' : case '=' : case '>' : case '<' : case '%' :
						l.push([2, s[i]]);
						break;
					case '(' : l.push([3, '']); break;
					case ')' : l.push([4, '']); break;
					case ',' : l.push([8, '']); break;
					case ':' : //objet de type sp�cial
						if(l_e[1] == 'f') //si c'est de type fonction
						{
							l.pop();
							l.push([7, '', 0]);
						}
						break;
					case '0' : case '1' : case '2' : case '3' : case '4' : 
					case '5' : case '6' : case '7' : case '8' : case '9' : 
						if(i == 0 || s[i-1] == ' ') //si pr�c�d� d'un espace : nouveau nombre
						{
							l.push([0, parseInt(s[i])]);
						}
						else if(l_e[0] == 0) //si pr�c�d� d'un nombre
						{
							l_e[1] = parseInt(s[i]) + l_e[1] * 10;
						}
						else if(l_e[0] == 5) //si pr�c�d� d'un nom de var
						{
							l_e[1] = l_e[1]+s[i];
						}
						else //sinon, c'est bien un nouveau nombre
						{
							l.push([0, parseInt(s[i])]);
						}
						break;
					default:
						if((i > 0 && s[i-1] != ' ') && l_e[0] == 5) //suite du nom de la variable pr�c�dente
						{
							l_e[1] = l_e[1]+s[i];
						}
						else
						{
							l.push([5, s[i], 1]);
						}
				}
			}
		}
		return l;
	};
	
	this.op1 = function(s, a)
	{
		switch(s)
		{
			case '-' : return -a; break;
			case '!' : return !a; break;
		}
	};
	
	this.op2 = function(s, a, b)
	{
		switch(s)
		{
			case '+' : return parseInt(a) + parseInt(b); break;
			case '-' : return a - b; break;
			case '*' : return a * b; break;
			case '/' : return Math.floor(a / b); break;
			case '%' : return a % b; break;
			case '^' : return Math.pow(a, b); break;
			case '.' : return a.toString() + b.toString(); break;
			case '&' : return a && b; break;
			case '|' : return a || b; break;
			case '=' : return a == b; break;
			case '!' : return a != b; break;
			case '>' : return a > b; break;
			case '<' : return a < b; break;
		}
	};
	
	this.prio = function(op)
	{
		switch(op)
		{
			case '&' : case '|' : return 1; break;
			case '>' : case '<' : case '=' : case '.' : return 2; break;
			case '+' : case '-' : return 3; break;
			case '*' : case '/' : case '%' : return 4; break;
			case '^' : return 5; break;
		}
	};
	
	/* type arbre
		[100, int] pour un nombre
		[101,str,arbre] pour un op unaire
		[102,str,arbre,arbre] pour un op binaire
		[105,str] pour une var
		[106,str] pour une str
		[107, str, arbre] pour une fct
		[108,array] pour un arbre d'arguments
	*/
	
	this.empile_syn = function(x, pile)
	{
		function est_a(a) //si �a existe et est un arbre
		{
			return (a && a[0] >= 100);
		}
		function est_l_op1(a) //si �a existe et est un op unaire
		{
			return (a && a[0] == 1);
		}
		function est_l_op2(a) //si �a existe et est un op binaire
		{
			return (a && a[0] == 2);
		}
		
		
		if(x[0] == 0) //si x est un nombre
		{
			return this.empile_syn([100, x[1]], pile);
		}
		else if(x[0] == 5) //si x est une var
		{
			return this.empile_syn([105, x[1]], pile);
		}
		else if(x[0] == 6) //si x est un texte
		{
			return this.empile_syn([106, x[1]], pile);
		}
		else if(est_a(x) && est_l_op1(pile[0])) // op1 x
		{
			var elt = pile.shift();
			return this.empile_syn([101, elt[1], x], pile);
		}
		else if(est_l_op2(x) && (est_a(pile[0]) && est_l_op2(pile[1]) && est_a(pile[2])) && (this.prio(x[1]) <= this.prio(pile[1][1]))) //a op2 b op2x
		{
			var b = pile.shift();
			var op = pile.shift();
			var a = pile.shift();
			pile.unshift([102, op[1], a, b]);
			return this.empile_syn(x, pile);
		}
		else if(x[0] == 4 && (est_a(pile[0]) && est_l_op2(pile[1]) && est_a(pile[2]))) //a op2 b )
		{
			var b = pile.shift();
			var op = pile.shift();
			var a = pile.shift();
			pile.unshift([102, op[1], a, b]);
			return this.empile_syn(x, pile);
		}
		else if(x[0] == 4 && (est_a(pile[0]) && pile[1] && pile[1][0] == 8 && est_a(pile[2]))) //arg_prec , a )
		{
			var a = pile.shift();
			pile.shift();
			var arg_prec = pile.shift();
			if(a[0] == 108) //si l'arbre en t�te de pile est d�j� une liste d'arguments
			{
				a[1].unshift(arg_prec);
				var elt = [108, a[1]];
			}
			else //sinon, on cr�e une nouvelle liste d'arguments
			{
				var elt = [108, [arg_prec, a]];
			}
			pile.unshift(elt);
			return this.empile_syn(x, pile);
		}
		else if(x[0] == 4 && (est_a(pile[0]) && pile[1] && pile[1][0] == 3 && pile[2] && pile[2][0] == 7)) //fct ( a )
		{
			var a = pile.shift();
			pile.shift();
			var f = pile.shift();
			var elt = [107, f[1], a];
			return this.empile_syn(elt, pile);
		}
		else if(x[0] == 4 && (pile[0] && pile[0][0] == 3 && pile[1] && pile[1][0] == 7)) //fct ( )
		{
			pile.shift();
			var f = pile.shift();
			var elt = [107, f[1], [108, []]];
			return this.empile_syn(elt, pile);
		}
		else if(x[0] == 4 && (est_a(pile[0]) && pile[1] && pile[1][0] == 3)) //( elt )
		{
			var elt = pile.shift();
			pile.shift();
			return this.empile_syn(elt, pile);
		}
		else
		{
			pile.unshift(x);
			return pile;
		}
	};
	
	this.parse_inf = function(pile, f)
	{
		if(f.length == 0)
		{
			return pile;
		}
		else
		{
			var x = f.shift();
			return this.parse_inf(this.empile_syn(x, pile), f);
		}
	};
	
	this.parse = function(ch)
	{
		function est_a(a) //si �a existe et est un arbre
		{
			return (a && a[0] >= 100);
		}
		var l = this.decompose(ch);
		if(l[0]) //s'il y a au moins un lexeme
		{
			l.push([4, '']);
			l.unshift([3, '']);
			var p = this.parse_inf(new Array(), l);
			if(est_a(p[0]))
			{
				return p[0];
			}
			else
			{
				throw "BadExpression:ParseError";
				return [106, ''];
			}
		}
		else
		{
			return [106, ''];
		}
	};
	
	this.eval_var = function(a, var_env)
	{
		return var_env.get(a);
	};
	
	this.custom_fcts = new Object();

	// Cache for parsed expression trees, keyed by expression string.
	// Since eval_syn does not mutate parse trees, cached trees are safe to reuse.
	this._parse_cache = new Object();

	this.eval_fct = function(s, args, var_env)
	{
		if(typeof(args) !='object' || !(args instanceof Array))
		{
			args = [args];
		}
		
		if(s in this.custom_fcts)
		{
			return this.custom_fcts[s].apply(undefined, args);
		}
		else
		{
		
			switch(s)
			{
				case 'str_begins_with' :
					if(args[0].toString().indexOf(args[1]) == 0)
					{
						return true;
					}
					else
					{
						return false;
					}
					break;
				case 'str_ends_with' :
					if(args[0].toString().lastIndexOf(args[1]) == (args[0].toString().length - args[1].toString().length))
					{
						return true;
					}
					else
					{
						return false;
					}
					break;
				case 'str_contains' :
					if(args[0].toString().indexOf(args[1]) != -1)
					{
						return true;
					}
					else
					{
						return false;
					}
					break;
				case 'str_first_word' :
					var mots = args[0].toString().split(' ');
					return mots[0];
					break;
				case 'str_distance' :
					var a = args[0].toString();
					var b = args[1].toString();
					var l;

					if(args[2] && args[2] == 1) //si sensible � la casse
					{
						l = levenshtein(a,b);
					}
					else
					{
						l = levenshtein(a.toLowerCase(),b.toLowerCase());
					}
					return l * 100 / Math.max(a.length, b.length);
					break;
				case 'random_int' :
					var min, max;
					
					if(args.length >= 2)
					{
						min = parseInt(args[0]);
						max = parseInt(args[1]);
					}
					else
					{
						min = 0;
						max = 100;
					}
					return Math.floor(Math.random() * (1 + max - min) + min);
					break;
				case 'get_date' :
					var date = new Date();
					return date.getTime();
					break;
				case 'str_to_lower' :
					return args[0].toString().toLowerCase();
					break;
				case 'str_to_upper' :
					return args[0].toString().toUpperCase();
					break;
				case 'get_var_val' :
					return var_env.get(args[0]);
					break;
				case 'str_replace' : // f:str_replace(a,b,c) a:string searched, b:string to search for, c:string to replace with
					if(args.length >= 2){
						var a = args[0].toString();
						var b = args[1].toString();
						if(args.length >= 3){
							var c = args[2].toString();
							return a.replace(b,c);
						} else {
							return a.replace(b,'');
						}
					} else {
						throw "BadExpression:NotEnoughArguments : " + s;
						return false;
					}
					break;
				case 'str_pad_start' : // f:str_pad_start(a,b,c) a: string to pad, b: pad with, c: length
					if(args.length >= 3){
						var a = args[0].toString();
						var b = args[1].toString();
						var c = parseInt(args[2]);
						if(!isNaN(c) && Number.isInteger(c)){
							return a.padStart(c,b);
						} else {
							throw "BadExpression:ExpectedInteger : " + s;
							return false;
						}
					} else {
						throw "BadExpression:NotEnoughArguments : " + s;
						return false;
					}
					break;
				case 'str_pad_end' : // f:str_pad_end(a,b,c) a: string to pad, b: pad with, c: length
					if(args.length >= 3){
						var a = args[0].toString();
						var b = args[1].toString();
						var c = parseInt(args[2]);
						if(!isNaN(c) && Number.isInteger(c)){
							return a.padEnd(c,b);
						} else {
							throw "BadExpression:ExpectedInteger : " + s;
							return false;
						}
					} else {
						throw "BadExpression:NotEnoughArguments : " + s;
						return false;
					}
					break;
				case 'str_length' :
					if(args.length >= 1){
						return args[0].toString().length;
					} else {
						throw "BadExpression:NotEnoughArguments : " + s;
						return false;
					}
					break;
				default:
					throw "BadExpression:UnknownFunction : " + s;
					break;

			}
		}
	};
	
	this.eval_syn = function(a, var_env)
	{
		if(a[0] == 100 || a[0] == 106) //si c'est un nombre ou une str
		{
			return a[1];
		}
		else if(a[0] == 102) //si c'est un op2
		{
			return this.op2(a[1], this.eval_syn(a[2], var_env), this.eval_syn(a[3], var_env));
		}
		else if(a[0] == 101) //si c'est un op1
		{
			return this.op1(a[1], this.eval_syn(a[2], var_env));
		}
		else if(a[0] == 105)
		{
			return this.eval_var(a[1], var_env);
		}
		else if(a[0] == 107)
		{
			return this.eval_fct(a[1], this.eval_syn(a[2], var_env), var_env);
		}
		else if(a[0] == 108)
		{
			var resultat = [];
			for(var i = 0; i < a[1].length; i++)
			{
				resultat.push(this.eval_syn(a[1][i], var_env));
			}
			return resultat;
		}
	};
	
	this.set_fct = function(name, fct)
	{
		this.custom_fcts[name] = fct;
	};
	
	this.eval = function(s, var_env)
	{
		var tree = this._parse_cache[s];
		if(tree === undefined)
		{
			tree = this.parse(s);
			this._parse_cache[s] = tree;
		}
		return this.eval_syn(tree, var_env);
	};
	
}
var c_f = new ExpressionEngine();

//EXPORTED VARIABLES


//EXPORTED FUNCTIONS
function evaluate_expression(expression, variable_environment)
{
	return c_f.eval(expression, variable_environment);
}

function register_custom_function(name, fct)
{
	c_f.set_fct(name, fct);
}

//END OF MODULE
Modules.complete('expression_engine');
